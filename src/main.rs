use clap::Parser;
use crossterm::cursor::MoveTo;
use crossterm::execute;
use crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use gix::bstr::ByteSlice;
use inquire::{MultiSelect, Text};
use std::fs;
use std::io::stdout;

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

fn main() {
    // let args = Args::parse();

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
    let mut commit_vec: Vec<String> = vec![];
    for info in walk {
        let commit = match info {
            Ok(i) => i.object(),
            Err(_) => {
                println!("Can't read informations from a commit");
                continue;
            }
        };
        match commit {
            Ok(c) => {
                let msg = c.message().unwrap();
                let title = msg.title.to_str().unwrap().replace("\n", " ");
                commit_vec.push(title);
            }
            Err(_) => {
                println!("Can't read informations from a commit");
                continue;
            }
        }
    }
    commit_vec.reverse();

    let old = fs::read_to_string("CHANGELOG.md").unwrap_or_default();
    let mut new = String::new();

    loop {
        let _ = execute!(
            stdout(),
            EnterAlternateScreen,
            Clear(ClearType::All),
            MoveTo(0, 0)
        );
        let ans = MultiSelect::new("Select commit to comment:", commit_vec.clone()).prompt();
        let done = match ans {
            Ok(a) => a,
            Err(_) => {
                let _ = execute!(stdout(), LeaveAlternateScreen);
                new.push_str(old.as_str());
                match fs::write("CHANGELOG.md", new) {
                    Ok(_) => return,
                    Err(e) => println!("Error while writing the changelog: {:?}", e),
                }
                return;
            }
        };
        let comment_prompt = Text::new("Comments: ").prompt();
        let comment = match comment_prompt {
            Ok(c) => c,
            Err(_) => continue,
        };
        new.push_str(format!("## {}\n\n", comment).as_str());
        for elem in done.iter() {
            new.push_str(format!("  - {}\n", elem).as_str());
        }
        new.push_str("\n\n");
        commit_vec.retain(|x| !done.contains(x));
        let _ = execute!(stdout(), LeaveAlternateScreen);
    }
}
