use anyhow::{Context, Result};
use clap::{Args, Parser};
use std::{
    env,
    fs::{self, File},
    io::{self, BufRead, BufReader},
    path::Path,
    process::{Command, Stdio},
};

#[derive(Parser)]
#[clap(bin_name = "tf")]
enum Commands {
    SetEnv(SetEnv),
    Plan(Plan),
    Apply(Apply),
    Destroy(Destroy),
    Init,
}

#[derive(Args)]
struct SetEnv {
    new_env: String,
}

fn set_env(opts: SetEnv) -> Result<()> {
    // Update .envrc
    let mut new_contents = String::new();
    {
        let envrc = File::open(".envrc").context("Cannot open envrc file")?;
        let reader = BufReader::new(envrc);

        for line in LineIterator::new(b'\n', reader) {
            let line = line.unwrap();
            let line = std::str::from_utf8(&line).context("envrc appears to not be valid UTF-8")?;
            if line.contains("export ENV") {
                // we can use \n, as we're expecting bash anyway.
                new_contents.push_str(&format!("export ENV={}\n", &opts.new_env));
            } else {
                new_contents.push_str(line);
            }
        }
    }
    fs::write(".envrc", new_contents).context("Cannot write envrc file changes")?;

    // remove .terraform()
    let terraform_dir = Path::new(".terraform");
    if terraform_dir.exists() {
        fs::remove_dir_all(terraform_dir).context("Unable to remove Terraform cache directory.")?;
    }

    println!("Run 'direnv allow' to load new env changes. Terraform will need to be init'd again.");

    Ok(())
}

#[derive(Args)]
struct Apply {
    #[clap(short = 'y')]
    auto_approve: bool,
}

fn apply(opts: Apply) -> Result<()> {
    check_env()?;
    let env = env::var("ENV").context("ENV var not set")?;
    let file = Path::new("envs").join(env).join("main.tfvars");
    let file_str = file.to_string_lossy();
    let mut args = vec!["destroy", "-var-file", &file_str];
    if opts.auto_approve {
        args.push("--auto-approve");
    }

    run_terraform(args)?;

    Ok(())
}

#[derive(Args)]
struct Plan {
    // additional_args: Option<Vec<String>>,
}

fn plan() -> Result<()> {
    check_env()?;
    let env = env::var("ENV").expect("env not set");
    let file = Path::new("envs").join(env).join("main.tfvars");

    run_terraform(["plan", "-var-file", &file.to_string_lossy()])?;

    Ok(())
}

#[derive(Args)]
struct Destroy {}

fn destroy() -> Result<()> {
    check_env()?;
    let env = env::var("ENV")?;
    let file = Path::new("envs").join(env).join("main.tfvars");

    run_terraform(["destroy", "-var-file", &file.to_string_lossy()])?;

    Ok(())
}

fn init() -> Result<()> {
    check_env()?;
    let env = env::var("ENV")?;
    let file = Path::new("envs").join(env).join("terraform_state.tfvars");

    run_terraform(["init", "-backend-config", &file.to_string_lossy()])?;

    Ok(())
}

// Helpers

fn check_env() -> Result<()> {
    env::var("ENV")?;
    env::var("AWS_PROFILE")?;

    Ok(())
}

fn run_terraform<'a>(args: impl IntoIterator<Item = &'a str>) -> Result<()> {
    Command::new("terraform")
        .args(args)
        .stdout(Stdio::inherit())
        .spawn()?
        .wait()?;

    Ok(())
}
fn main() -> Result<()> {
    let args = Commands::parse();

    match args {
        Commands::SetEnv(a) => set_env(a)?,
        Commands::Apply(a) => apply(a)?,
        Commands::Plan(_) => plan()?,
        Commands::Destroy(_) => destroy()?,
        Commands::Init => init()?,
    }

    Ok(())
}

struct LineIterator<T: BufRead> {
    delimiter: u8,
    reader: T,
}

impl<T: BufRead> LineIterator<T> {
    fn new(delimiter: u8, reader: T) -> Self {
        Self { delimiter, reader }
    }
}

impl<T: BufRead> Iterator for LineIterator<T> {
    type Item = io::Result<Vec<u8>>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = Vec::new();
        match self.reader.read_until(self.delimiter, &mut buf) {
            Ok(0) => None,
            Ok(_) => Some(Ok(buf)),
            Err(e) => Some(Err(e)),
        }
    }
}
