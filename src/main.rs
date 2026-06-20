use clap::Parser;
use crossterm::cursor::MoveTo;
use crossterm::execute;
use crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use gix::bstr::ByteSlice;
use inquire::{MultiSelect, Text};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::io::stdout;
use std::{fmt, fs};

#[derive(Parser)]
#[command(
    name = "dogma",
    version,
    about = "Generate changelogs based on the commits"
)]
struct Args {
    #[arg(long)]
    from: Option<String>,

    #[arg(long, default_value = "HEAD")]
    to: String,
}

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
    // let args = Args::parse();
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

    loop {
        let old = fs::read_to_string("CHANGELOG.md").unwrap_or_default();
        let mut new = String::new();
        let _ = execute!(
            stdout(),
            EnterAlternateScreen,
            Clear(ClearType::All),
            MoveTo(0, 0)
        );
        let ans = MultiSelect::new("Select commit to comment:", commits.clone()).prompt();
        let done = match ans {
            Ok(a) => a,
            Err(_) => {
                let _ = execute!(stdout(), LeaveAlternateScreen);
                return;
            }
        };
        let version_prompt = Text::new("Version:").prompt();
        let version = match version_prompt {
            Ok(v) => v,
            Err(_) => continue,
        };
        let comment_prompt = Text::new("Comments:").prompt();
        let comment = match comment_prompt {
            Ok(c) => c,
            Err(_) => continue,
        };
        new.push_str(format!("## [{}] - {}\n\n", version, date).as_str());
        new.push_str(format!("  - {}\n", comment).as_str());
        for elem in done.iter() {
            new.push_str(format!("    - {}\n", elem).as_str());
            save_dogma(elem.id.clone());
        }
        new.push_str("\n");
        new.push_str(old.as_str());
        match fs::write("CHANGELOG.md", new) {
            Ok(_) => (),
            Err(e) => println!("Error while writing the changelog: {:?}", e),
        }
        commits.retain(|c| !done.iter().any(|d| d.id == c.id));
        let _ = execute!(stdout(), LeaveAlternateScreen);
    }
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
