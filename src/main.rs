use crossterm::cursor::MoveTo;
use crossterm::execute;
use crossterm::style::Stylize;
use crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use gix::bstr::ByteSlice;
use inquire::{Confirm, MultiSelect, Text};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::io::stdout;
use std::process::Command;
use std::{fmt, fs};

#[derive(Clone)]
struct Commit {
    id: String,
    title: String,
}

impl fmt::Display for Commit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", &self.id[..7.min(self.id.len())], self.title)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let sermon = args.iter().any(|a| a == "sermon");
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();

    let repo = match gix::discover(".") {
        Ok(r) => r,
        Err(_) => {
            println!("Not a git repository");
            return;
        }
    };

    let head_id = match repo.head_id() {
        Ok(it) => it,
        Err(_) => {
            println!("Can't find the HEAD id of this repository");
            return;
        }
    };
    let walk = match repo.rev_walk([head_id.detach()]).all() {
        Ok(w) => w,
        Err(_) => {
            println!("Can't walk through commits");
            return;
        }
    };
    let mut commits: Vec<Commit> = vec![];

    for info in walk {
        let info = match info {
            Ok(i) => i,
            Err(_) => {
                println!("Can't read information from a commit");
                continue;
            }
        };
        let id = info.id.to_string();
        match info.object() {
            Ok(c) => {
                let msg = c.message().unwrap();
                let title = msg.title.to_str().unwrap().replace("\n", " ");
                commits.push(Commit {
                    id,
                    title: title.clone(),
                });
            }
            Err(_) => {
                println!("Can't read information from a commit");
                continue;
            }
        }
    }
    commits.reverse();

    let already_done = load_dogma();
    commits.retain(|c| !already_done.contains(&c.id));
    let git_url = remote_base_url();

    if commits.len() == 0 {
        println!("No more commits to comments");
    }

    loop {
        let old = fs::read_to_string("CHANGELOG.md").unwrap_or_default();
        let mut new = String::new();
        let _ = execute!(
            stdout(),
            EnterAlternateScreen,
            Clear(ClearType::All),
            MoveTo(0, 0)
        );
        println!("{}", "Dogma - preach your releases".magenta().bold());
        let done;
        if sermon {
            done = commits.clone();
        } else {
            let commit_selected =
                MultiSelect::new("Select commit to comment:", commits.clone()).prompt();
            done = match commit_selected {
                Ok(a) => a,
                Err(_) => {
                    let _ = execute!(stdout(), LeaveAlternateScreen);
                    break;
                }
            };
            if done.len() == 0 {
                continue;
            }
        }
        let trash_prompt = Confirm::new("Do you want to trash these commit ?")
            .with_default(false)
            .prompt();
        match trash_prompt {
            Ok(true) => {
                for elem in done.iter() {
                    save_dogma(elem.id.clone());
                    continue;
                }
            }
            Ok(false) => {
                let version_prompt = Text::new("Version:").prompt();
                let version = match version_prompt {
                    Ok(v) => v,
                    Err(_) => {
                        if sermon {
                            break;
                        } else {
                            continue;
                        }
                    }
                };
                let comment_prompt = Text::new("Comments:").prompt();
                let comment = match comment_prompt {
                    Ok(c) => c,
                    Err(_) => {
                        if sermon {
                            break;
                        } else {
                            continue;
                        }
                    }
                };
                new.push_str(format!("## [{}] - {}\n\n", version, date).as_str());
                new.push_str(format!("  - {}\n", comment).as_str());
                for elem in done.iter() {
                    let short = &elem.id[..7.min(elem.id.len())];
                    let line = match &git_url {
                        Some(b) => format!(
                            "    - [{}]({}) {}\n",
                            short,
                            commit_url(b, &elem.id),
                            elem.title
                        ),
                        None => format!("    - {}\n", elem),
                    };
                    new.push_str(&line);
                    save_dogma(elem.id.clone());
                }
                new.push_str("\n");
                new.push_str(old.as_str());
                match fs::write("CHANGELOG.md", new) {
                    Ok(_) => (),
                    Err(e) => println!("Error while writing the changelog: {:?}", e),
                }
            }
            Err(_) => {
                if sermon {
                    break;
                } else {
                    continue;
                }
            }
        }
        commits.retain(|c| !done.iter().any(|d| d.id == c.id));
        if sermon {
            break;
        }
        let _ = execute!(stdout(), LeaveAlternateScreen);
    }
    let _ = execute!(stdout(), LeaveAlternateScreen);
}

fn load_dogma() -> HashSet<String> {
    fs::read_to_string(".dogma")
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn save_dogma(new: String) {
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(".dogma")
        .expect("Can't open .dogma");
    let _ = writeln!(f, "{}", new);
}

fn remote_base_url() -> Option<String> {
    let out = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Some(normalize_remote(&raw))
}

fn normalize_remote(raw: &str) -> String {
    let mut url = raw.to_string();

    if let Some(rest) = url.strip_prefix("git@") {
        url = format!("https://{}", rest.replacen(':', "/", 1));
    } else if let Some(rest) = url.strip_prefix("ssh://git@") {
        url = format!("https://{}", rest);
    }
    url = url.strip_suffix(".git").unwrap_or(&url).to_string();
    url.trim_end_matches('/').to_string()
}

fn commit_url(base: &str, hash: &str) -> String {
    let path = if base.contains("gitlab") {
        "/-/commit/"
    } else if base.contains("bitbucket") {
        "/commits/"
    } else {
        "/commit/"
    };
    format!("{}{}{}", base, path, hash)
}
