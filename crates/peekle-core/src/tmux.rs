//! Finding the tmux pane a session lives in, and typing into it.
//!
//! This is the fast half of the delivery ladder in tech.md 6.5: where a pane
//! exists, text goes in as keystrokes, immediately, and no turn is held.
//!
//! Everything that talks to `tmux` is behind [`Tmux`], so the pure parts —
//! argument building, pane matching, process-tree walking — stay testable
//! without a tmux server running.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use crate::types::TmuxTarget;

/// Where tmux might be. Same reasoning as `claude_path`: a Finder-launched app
/// does not inherit the shell's PATH. tech.md 6.4.
const TMUX_CANDIDATES: &[&str] = &[
    "/opt/homebrew/bin/tmux",
    "/usr/local/bin/tmux",
    "/usr/bin/tmux",
];

/// The format `list-panes` is asked for: target first, pane pid second.
const PANE_FORMAT: &str = "#{session_name}:#{window_index}.#{pane_index} #{pane_pid}";

pub fn tmux_path() -> Option<PathBuf> {
    TMUX_CANDIDATES
        .iter()
        .map(PathBuf::from)
        .find(|path| path.exists())
}

/// One `session:window.pane` and the pid of the process running in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane {
    pub target: TmuxTarget,
    pub pid: u32,
}

/// Parses the output of `list-panes -F PANE_FORMAT`.
///
/// Malformed lines are skipped rather than failing the batch: one pane whose
/// name confuses the parser must not cost us every other pane.
pub fn parse_panes(output: &str) -> Vec<Pane> {
    output
        .lines()
        .filter_map(|line| {
            let (target, pid) = line.trim().rsplit_once(' ')?;
            Some(Pane {
                target: TmuxTarget::parse(target)?,
                pid: pid.parse().ok()?,
            })
        })
        .collect()
}

/// Walks up from `pid` through `parents` looking for any pane pid.
///
/// The agent is never the pane process itself: the pane runs a shell, the shell
/// runs `claude`, and with a wrapper or a `sudo` in between the chain is longer
/// still. So the question is ancestry, not equality.
///
/// The walk is bounded by the size of the map, which makes a corrupted parent
/// map that points in a circle terminate instead of hanging the caller.
pub fn pane_for_pid(pid: u32, panes: &[Pane], parents: &HashMap<u32, u32>) -> Option<TmuxTarget> {
    let mut current = pid;
    for _ in 0..=parents.len() {
        if let Some(pane) = panes.iter().find(|pane| pane.pid == current) {
            return Some(pane.target.clone());
        }
        match parents.get(&current) {
            // pid 1 parents itself in some listings; either way we are done.
            Some(&parent) if parent != current && parent != 0 => current = parent,
            _ => return None,
        }
    }
    None
}

/// Parses `ps -Ao pid=,ppid=` into a child -> parent map.
pub fn parse_process_tree(output: &str) -> HashMap<u32, u32> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid = parts.next()?.parse().ok()?;
            let ppid = parts.next()?.parse().ok()?;
            Some((pid, ppid))
        })
        .collect()
}

/// The two commands that put one line of text into a pane. tech.md 6.5.
///
/// `-l` is not optional: without it tmux reads the payload as key names, so a
/// message containing `Enter` or `C-c` would arrive as something other than
/// what the user typed. Enter goes as its own command precisely because in
/// literal mode it would be five characters rather than a key.
pub fn send_keys_args(target: &TmuxTarget, text: &str) -> [Vec<String>; 2] {
    let target = target.target();
    [
        vec![
            "send-keys".to_string(),
            "-t".to_string(),
            target.clone(),
            "-l".to_string(),
            text.to_string(),
        ],
        vec![
            "send-keys".to_string(),
            "-t".to_string(),
            target,
            "Enter".to_string(),
        ],
    ]
}

/// The live tmux binary. Everything above this line is pure.
pub struct Tmux {
    path: PathBuf,
}

impl Tmux {
    /// None when tmux is not installed, which is the common case and not an
    /// error: it just means every session takes the turn-boundary path.
    pub fn find() -> Option<Self> {
        tmux_path().map(|path| Self { path })
    }

    fn run(&self, args: &[String]) -> Option<String> {
        let output = Command::new(&self.path).args(args).output().ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8(output.stdout).ok()
    }

    pub fn panes(&self) -> Vec<Pane> {
        let args = [
            "list-panes".to_string(),
            "-a".to_string(),
            "-F".to_string(),
            PANE_FORMAT.to_string(),
        ];
        self.run(&args)
            .map(|out| parse_panes(&out))
            .unwrap_or_default()
    }

    /// The pane whose process is an ancestor of `pid`.
    ///
    /// Deliberately no fallback to matching on the working directory: two
    /// sessions in one checkout are ordinary, and picking between them by path
    /// would type one user's message into the other user's agent.
    pub fn pane_for(&self, pid: u32) -> Option<TmuxTarget> {
        let panes = self.panes();
        if panes.is_empty() {
            return None;
        }
        pane_for_pid(pid, &panes, &process_tree())
    }

    /// Types `text` into `target` and presses Enter.
    ///
    /// A true return means tmux accepted the keys, not that anybody read them:
    /// send-keys succeeds against a pane whose Claude Code has already exited.
    /// Delivery is confirmed by `UserPromptSubmit`, never by this. tech.md 6.3.
    pub fn send(&self, target: &TmuxTarget, text: &str) -> bool {
        send_keys_args(target, text)
            .iter()
            .all(|args| self.run(args).is_some())
    }
}

/// The whole process table as child -> parent.
pub fn process_tree() -> HashMap<u32, u32> {
    let Ok(output) = Command::new("/bin/ps").args(["-Ao", "pid=,ppid="]).output() else {
        return HashMap::new();
    };
    String::from_utf8(output.stdout)
        .map(|out| parse_process_tree(&out))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(session: &str, window: u32, pane: u32) -> TmuxTarget {
        TmuxTarget {
            session: session.to_string(),
            window,
            pane,
        }
    }

    #[test]
    fn parses_what_list_panes_prints() {
        let out = "work:0.1 4242\nmain:2.0 99\n";
        assert_eq!(
            parse_panes(out),
            vec![
                Pane {
                    target: target("work", 0, 1),
                    pid: 4242
                },
                Pane {
                    target: target("main", 2, 0),
                    pid: 99
                },
            ]
        );
    }

    /// A session name may contain a colon, so the split has to come from the
    /// right. Splitting from the left renamed the session and lost the pane.
    #[test]
    fn session_names_may_contain_a_colon() {
        let panes = parse_panes("feat:api:1.2 7\n");
        assert_eq!(panes[0].target.session, "feat:api");
        assert_eq!(panes[0].target.window, 1);
        assert_eq!(panes[0].target.pane, 2);
    }

    #[test]
    fn skips_malformed_lines_without_losing_the_rest() {
        let panes = parse_panes("garbage\nwork:0.1 4242\nalso bad\n\n");
        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].pid, 4242);
    }

    #[test]
    fn target_string_is_what_tmux_takes() {
        assert_eq!(target("work", 3, 2).target(), "work:3.2");
    }

    /// The agent is a grandchild of the pane: pane -> shell -> claude.
    #[test]
    fn finds_the_pane_through_the_process_tree() {
        let panes = vec![Pane {
            target: target("work", 0, 1),
            pid: 100,
        }];
        let parents = HashMap::from([(300, 200), (200, 100), (100, 1)]);
        assert_eq!(
            pane_for_pid(300, &panes, &parents),
            Some(target("work", 0, 1))
        );
    }

    #[test]
    fn a_pid_outside_every_pane_has_no_target() {
        let panes = vec![Pane {
            target: target("work", 0, 1),
            pid: 100,
        }];
        let parents = HashMap::from([(500, 400), (400, 1)]);
        assert_eq!(pane_for_pid(500, &panes, &parents), None);
    }

    /// A parent map that points in a circle must not hang the island.
    #[test]
    fn a_cyclic_parent_map_terminates() {
        let panes = vec![Pane {
            target: target("work", 0, 1),
            pid: 100,
        }];
        let parents = HashMap::from([(10, 11), (11, 12), (12, 10)]);
        assert_eq!(pane_for_pid(10, &panes, &parents), None);
    }

    #[test]
    fn parses_the_process_table() {
        let tree = parse_process_tree("  300   200\n  200     1\n");
        assert_eq!(tree.get(&300), Some(&200));
        assert_eq!(tree.get(&200), Some(&1));
    }

    /// Literal mode and a separate Enter, or the message arrives as key names.
    #[test]
    fn send_keys_sends_text_literally() {
        let [text, enter] = send_keys_args(&target("work", 0, 1), "press Enter now");
        assert_eq!(
            text,
            vec!["send-keys", "-t", "work:0.1", "-l", "press Enter now"]
        );
        assert_eq!(enter, vec!["send-keys", "-t", "work:0.1", "Enter"]);
    }

    /// Every one of these used to be a way to lose or mangle a message. They
    /// ride in one argv slot, so the shell never sees them. tech.md S3.
    #[test]
    fn awkward_text_survives_intact() {
        for text in [
            "rm -rf / # not really",
            "echo \"quoted\" and 'single'",
            "C-c",
            "line one\nline two",
            "путь с пробелами и эмодзи 🚀",
            "$(whoami) `id` ${HOME}",
            "trailing space ",
        ] {
            let [args, _] = send_keys_args(&target("s", 0, 0), text);
            assert_eq!(args.last().unwrap(), text, "mangled: {text}");
            assert_eq!(args.len(), 5);
        }
    }
}
