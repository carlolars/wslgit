use std::borrow::Cow;
use std::env;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Component, Path, Prefix, PrefixComponent};
use std::process::{Command, Stdio};

#[macro_use]
extern crate lazy_static;
extern crate regex;
use regex::bytes::Regex;

fn get_drive_letter(pc: &PrefixComponent) -> Option<String> {
    let drive_byte = match pc.kind() {
        Prefix::VerbatimDisk(d) => Some(d),
        Prefix::Disk(d) => Some(d),
        _ => None,
    };
    drive_byte.map(|drive_letter| {
        String::from_utf8(vec![drive_letter])
            .expect(&format!("Invalid drive letter: {}", drive_letter))
            .to_lowercase()
    })
}

fn mount_root() -> String {
    match env::var("WSLGIT_MOUNT_ROOT") {
        Ok(val) => {
            if val.ends_with("/") {
                return val;
            } else {
                return format!("{}/", val);
            }
        }
        Err(_e) => return "/mnt/".to_string(),
    }
}

fn get_prefix_for_drive(drive: &str) -> String {
    format!("{}{}", mount_root(), drive)
}

fn translate_path_to_unix(argument: String) -> String {
    let argument = patch_argument_for_fork(argument);
    {
        let (argname, arg) = if argument.contains('=') {
            let parts: Vec<&str> = argument.splitn(2, '=').collect();
            (format!("{}=", parts[0]), parts[1])
        } else {
            ("".to_owned(), argument.as_ref())
        };
        let win_path = Path::new(arg);
        if win_path.is_absolute() || win_path.exists() {
            let wsl_path: String = win_path.components().fold(String::new(), |mut acc, c| {
                match c {
                    Component::Prefix(prefix_comp) => {
                        let d = get_drive_letter(&prefix_comp)
                            .expect(&format!("Cannot handle path {:?}", win_path));
                        acc.push_str(&get_prefix_for_drive(&d));
                    }
                    Component::RootDir => {}
                    _ => {
                        let d = c
                            .as_os_str()
                            .to_str()
                            .expect(&format!("Cannot represent path {:?}", win_path))
                            .to_owned();
                        if !acc.is_empty() && !acc.ends_with('/') {
                            acc.push('/');
                        }
                        acc.push_str(&d);
                    }
                };
                acc
            });

            return format!("{}{}", &argname, &wsl_path);
        }
    }
    argument
}

fn patch_argument_for_fork(path: String) -> String {
    // "xxx.editor=C:/Users/xxx/AppData/Local/Fork/app-x.xx.x/Fork.RI.exe"
    lazy_static! {
        static ref FORK_RI_EXE_PATH_EX: regex::Regex = regex::Regex::new(
            // r"\.editor=(?P<fork_ri_exe_path>.*Fork\.RI\.exe)"
            r"(?P<prefix>\.editor=)(?P<fork_ri_exe_path>.*Fork\.RI\.exe)"
        )
        .expect("Failed to compile FORK_RI_EXE_PATH_EX regex");
    }

    match FORK_RI_EXE_PATH_EX.captures(path.as_str()) {
        Some(caps) => {
            let fork_ri_exe_path = caps.name("fork_ri_exe_path").unwrap().as_str();
            pass_value_to_wsl("FORK_RI_EXE_PATH", fork_ri_exe_path);

            let fork_ri_script_path = match env::current_exe() {
                Ok(p) => p
                    .parent()
                    .unwrap()
                    .join("Fork.RI")
                    .to_string_lossy()
                    .into_owned(),
                Err(e) => {
                    eprintln!("Failed to get current exe path: {}", e);
                    panic!();
                }
            };
            let r = format!("${{prefix}}{}", fork_ri_script_path);
            return FORK_RI_EXE_PATH_EX
                .replace_all(path.as_str(), r.as_str())
                .into_owned();
        }
        None => return path,
    }
}

/// Pass a value to WSL by using an environment variable and WSLENV.
///
/// * `name` - Name to use for the environment variable.
/// * `value` - The value to pass to WSL using the environment variable.
fn pass_value_to_wsl(name: &str, value: &str) {
    if env::var(name).is_err() {
        env::set_var(name, value);
    }

    match env::var("WSLENV") {
        Ok(wslenv) => {
            // WSLENV exists, add new variable only once
            let re: regex::Regex = regex::Regex::new(format!(r"(^|:){}(/|:|$)", name).as_str())
                .expect("Failed to compile regex");
            if re.is_match(wslenv.as_str()) == false {
                let wslenv = format!("{}:{}/p", wslenv, name);
                env::set_var("WSLENV", wslenv);
            }
        }
        Err(_e) => {
            // No WSLENV
            let wslenv = format!("{}/p", name);
            env::set_var("WSLENV", wslenv);
        }
    };
}

// Translate absolute unix paths to windows paths by mapping what looks like a mounted drive ('/mnt/x') to a drive letter ('x:/').
// The path must either be the start of a line or start with a whitespace, and
// the path must be the end of a line, end with a / or end with a whitespace.
fn translate_path_to_win(line: &[u8]) -> Cow<[u8]> {
    let wslpath_re: Regex = Regex::new(
        format!(
            r"(?m-u)(^|(?P<pre>[[:space:]])){}(?P<drive>[A-Za-z])($|/|(?P<post>[[:space:]]))",
            mount_root()
        )
        .as_str(),
    )
    .expect("Failed to compile WSLPATH regex");

    wslpath_re.replace_all(line, &b"${pre}${drive}:/${post}"[..])
}

fn escape_newline(arg: String) -> String {
    arg.replace("\n", "$'\n'")
}

fn quote_characters(ch: char) -> bool {
    match ch {
        '\"' | '\'' => true,
        _ => false,
    }
}

fn invalid_characters(ch: char) -> bool {
    match ch {
        ' ' | '(' | ')' | '|' => true,
        _ => false,
    }
}

fn format_argument(arg: String) -> String {
    if arg.contains(quote_characters) {
        // if argument contains quotes then assume it is correctly quoted.
        return arg;
    } else if arg.contains(invalid_characters) || arg.is_empty() {
        return format!("\"{}\"", arg);
    } else {
        return arg;
    }
}

fn use_interactive_shell() -> bool {
    // check for explicit environment variable setting
    if let Ok(interactive_flag) = env::var("WSLGIT_USE_INTERACTIVE_SHELL") {
        if interactive_flag == "false" || interactive_flag == "0" {
            return false;
        } else {
            return true;
        }
    }
    // check for advanced usage indicated by BASH_ENV and WSLENV contains BASH_ENV
    else if env::var("BASH_ENV").is_ok() {
        if let Ok(wslenv) = env::var("WSLENV") {
            lazy_static! {
                // BASH_ENV can be first or after another variable.
                // It can be followed by flags, another variable or be last.
                static ref BASH_ENV_RE: Regex = Regex::new(r"(?-u)(^|:)BASH_ENV(/|:|$)")
                    .expect("Failed to compile BASH_ENV regex");
            }
            if BASH_ENV_RE.is_match(wslenv.as_bytes()) {
                return false;
            }
        }
    }
    true
}

fn enable_logging() -> bool {
    if let Ok(enable_log_flag) = env::var("WSLGIT_ENABLE_LOGGING") {
        if enable_log_flag == "true" || enable_log_flag == "1" {
            return true;
        }
    }
    false
}

fn log_arguments(out_args: &Vec<String>) {
    let in_args = env::args().collect::<Vec<String>>();
    let logfile = match env::current_exe() {
        Ok(exe_path) => exe_path
            .parent()
            .unwrap()
            .join("wslgit.log")
            .to_string_lossy()
            .into_owned(),
        Err(e) => {
            eprintln!("Failed to get current exe path: {}", e);
            Path::new("wslgit.log").to_string_lossy().into_owned()
        }
    };

    let f = OpenOptions::new()
        .append(true)
        .create(true)
        .open(logfile)
        .unwrap();
    write!(&f, "{:?} -> {:?}\n", in_args, out_args).unwrap();
}

fn main() {
    let mut cmd_args = Vec::new();
    let cwd_unix =
        translate_path_to_unix(env::current_dir().unwrap().to_string_lossy().into_owned());
    let mut git_args: Vec<String> = vec![
        String::from("cd"),
        format!("\"{}\"", cwd_unix),
        String::from("&&"),
        String::from("git"),
    ];

    git_args.extend(
        env::args()
            .skip(1)
            .map(translate_path_to_unix)
            .map(format_argument)
            .map(escape_newline),
    );

    let git_cmd: String = git_args.join(" ");

    // build the command arguments that are passed to wsl.exe
    cmd_args.push("bash".to_string());
    if use_interactive_shell() {
        cmd_args.push("-ic".to_string());
    } else {
        cmd_args.push("-c".to_string());
    }
    cmd_args.push(git_cmd.clone());

    if enable_logging() {
        log_arguments(&cmd_args);
    }

    // setup the git subprocess launched inside WSL
    let mut git_proc_setup = Command::new("wsl");
    git_proc_setup.args(&cmd_args);
    let status;

    // add git commands that must use translate_path_to_win
    const TRANSLATED_CMDS: &[&str] = &["rev-parse", "remote"];

    let translate_output = env::args()
        .skip(1)
        .position(|arg| {
            TRANSLATED_CMDS
                .iter()
                .position(|&tcmd| tcmd == arg)
                .is_some()
        })
        .is_some();

    if translate_output {
        // run the subprocess and capture its output
        let git_proc = git_proc_setup
            .stdout(Stdio::piped())
            .spawn()
            .expect(&format!("Failed to execute command '{}'", &git_cmd));
        let output = git_proc
            .wait_with_output()
            .expect(&format!("Failed to wait for git call '{}'", &git_cmd));
        status = output.status;
        let output_bytes = output.stdout;
        let mut stdout = io::stdout();
        stdout
            .write_all(&translate_path_to_win(&output_bytes))
            .expect("Failed to write git output");
        stdout.flush().expect("Failed to flush output");
    } else {
        // run the subprocess without capturing its output
        // the output of the subprocess is passed through unchanged
        status = git_proc_setup
            .status()
            .expect(&format!("Failed to execute command '{}'", &git_cmd));
    }

    // forward any exit code
    if let Some(exit_code) = status.code() {
        std::process::exit(exit_code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_root_test() {
        env::remove_var("WSLGIT_MOUNT_ROOT");
        assert_eq!(mount_root(), "/mnt/");

        env::set_var("WSLGIT_MOUNT_ROOT", "/abc/");
        assert_eq!(mount_root(), "/abc/");

        env::set_var("WSLGIT_MOUNT_ROOT", "/abc");
        assert_eq!(mount_root(), "/abc/");

        env::set_var("WSLGIT_MOUNT_ROOT", "/");
        assert_eq!(mount_root(), "/");
    }

    #[test]
    fn use_interactive_shell_test() {
        // default
        env::remove_var("WSLGIT_USE_INTERACTIVE_SHELL");
        env::remove_var("BASH_ENV");
        env::remove_var("WSLENV");
        assert_eq!(use_interactive_shell(), true);

        // disable using WSLGIT_USE_INTERACTIVE_SHELL set to 'false' or '0'
        env::set_var("WSLGIT_USE_INTERACTIVE_SHELL", "false");
        assert_eq!(use_interactive_shell(), false);
        env::set_var("WSLGIT_USE_INTERACTIVE_SHELL", "0");
        assert_eq!(use_interactive_shell(), false);

        // enable using WSLGIT_USE_INTERACTIVE_SHELL set to anything but 'false' and '0'
        env::set_var("WSLGIT_USE_INTERACTIVE_SHELL", "true");
        assert_eq!(use_interactive_shell(), true);
        env::set_var("WSLGIT_USE_INTERACTIVE_SHELL", "1");
        assert_eq!(use_interactive_shell(), true);

        env::remove_var("WSLGIT_USE_INTERACTIVE_SHELL");

        // just having BASH_ENV is not enough
        env::set_var("BASH_ENV", "something");
        assert_eq!(use_interactive_shell(), true);

        // BASH_ENV must also be in WSLENV
        env::set_var("WSLENV", "BASH_ENV");
        assert_eq!(use_interactive_shell(), false);
        env::set_var("WSLENV", "BASH_ENV/up");
        assert_eq!(use_interactive_shell(), false);
        env::set_var("WSLENV", "BASH_ENV:TMP");
        assert_eq!(use_interactive_shell(), false);
        env::set_var("WSLENV", "BASH_ENV/up:TMP");
        assert_eq!(use_interactive_shell(), false);
        env::set_var("WSLENV", "TMP:BASH_ENV");
        assert_eq!(use_interactive_shell(), false);
        env::set_var("WSLENV", "TMP:BASH_ENV/up");
        assert_eq!(use_interactive_shell(), false);
        env::set_var("WSLENV", "TMP:BASH_ENV:TMP");
        assert_eq!(use_interactive_shell(), false);
        env::set_var("WSLENV", "TMP:BASH_ENV/up:TMP");
        assert_eq!(use_interactive_shell(), false);

        env::set_var("WSLENV", "NOT_BASH_ENV/up");
        assert_eq!(use_interactive_shell(), true);

        // WSLGIT_USE_INTERACTIVE_SHELL overrides BASH_ENV
        env::set_var("WSLGIT_USE_INTERACTIVE_SHELL", "true");
        assert_eq!(use_interactive_shell(), true);
    }

    #[test]
    fn escape_newline() {
        assert_eq!(
            super::escape_newline("ab\ncdef".to_string()),
            "ab$\'\n\'cdef"
        );
        assert_eq!(
            super::escape_newline("ab\ncd ef".to_string()),
            "ab$\'\n\'cd ef"
        );
        // Long arguments with newlines...
        assert_eq!(
            super::escape_newline("--ab\ncdef".to_string()),
            "--ab$\'\n\'cdef"
        );
        assert_eq!(
            super::escape_newline("--ab\ncd ef".to_string()),
            "--ab$\'\n\'cd ef"
        );
    }

    #[test]
    fn format_argument_with_invalid_character() {
        assert_eq!(format_argument("abc def".to_string()), "\"abc def\"");
        assert_eq!(format_argument("abc(def".to_string()), "\"abc(def\"");
        assert_eq!(format_argument("abc)def".to_string()), "\"abc)def\"");
        assert_eq!(format_argument("abc|def".to_string()), "\"abc|def\"");
        assert_eq!(format_argument("\"abc def\"".to_string()), "\"abc def\"");
        assert_eq!(
            format_argument("user.(name|email)".to_string()),
            "\"user.(name|email)\""
        );
    }

    #[test]
    fn format_long_argument_with_invalid_character() {
        assert_eq!(format_argument("--abc def".to_string()), "\"--abc def\"");
        assert_eq!(format_argument("--abc=def".to_string()), "--abc=def");
        assert_eq!(format_argument("--abc=d ef".to_string()), "\"--abc=d ef\"");
        assert_eq!(format_argument("--abc=d(ef".to_string()), "\"--abc=d(ef\"");
        assert_eq!(format_argument("--abc=d)ef".to_string()), "\"--abc=d)ef\"");
        assert_eq!(format_argument("--abc=d|ef".to_string()), "\"--abc=d|ef\"");
        assert_eq!(
            format_argument("--pretty=format:a(b|c)d".to_string()),
            "\"--pretty=format:a(b|c)d\""
        );
        assert_eq!(
            format_argument("--pretty=format:a (b | c) d".to_string()),
            "\"--pretty=format:a (b | c) d\""
        );
        // Long arguments with invalid characters in argument name
        assert_eq!(format_argument("--abc(def".to_string()), "\"--abc(def\"");
        assert_eq!(format_argument("--abc)def".to_string()), "\"--abc)def\"");
        assert_eq!(format_argument("--abc|def".to_string()), "\"--abc|def\"");
    }

    #[test]
    fn format_empty_argument() {
        assert_eq!(format_argument("".to_string()), "\"\"");
    }

    #[test]
    fn win_to_unix_path_trans() {
        env::remove_var("WSLGIT_MOUNT_ROOT");
        assert_eq!(
            translate_path_to_unix("d:\\test\\file.txt".to_string()),
            "/mnt/d/test/file.txt"
        );
        assert_eq!(
            translate_path_to_unix("C:\\Users\\test\\a space.txt".to_string()),
            "/mnt/c/Users/test/a space.txt"
        );

        env::set_var("WSLGIT_MOUNT_ROOT", "/abc/");
        assert_eq!(
            translate_path_to_unix("d:\\test\\file.txt".to_string()),
            "/abc/d/test/file.txt"
        );
        assert_eq!(
            translate_path_to_unix("C:\\Users\\test\\a space.txt".to_string()),
            "/abc/c/Users/test/a space.txt"
        );

        env::set_var("WSLGIT_MOUNT_ROOT", "/");
        assert_eq!(
            translate_path_to_unix("d:\\test\\file.txt".to_string()),
            "/d/test/file.txt"
        );
        assert_eq!(
            translate_path_to_unix("C:\\Users\\test\\a space.txt".to_string()),
            "/c/Users/test/a space.txt"
        );
    }

    #[test]
    fn unix_to_win_path_trans() {
        env::remove_var("WSLGIT_MOUNT_ROOT");
        assert_eq!(
            &*translate_path_to_win(b"/mnt/d/some path/a file.md"),
            b"d:/some path/a file.md"
        );
        assert_eq!(
            &*translate_path_to_win(b"origin  /mnt/c/path/ (fetch)"),
            b"origin  c:/path/ (fetch)"
        );
        let multiline = b"mirror  /mnt/c/other/ (fetch)\nmirror  /mnt/c/other/ (push)\n";
        let multiline_result = b"mirror  c:/other/ (fetch)\nmirror  c:/other/ (push)\n";
        assert_eq!(
            &*translate_path_to_win(&multiline[..]),
            &multiline_result[..]
        );
        assert_eq!(
            &*translate_path_to_win(b"/mnt/c  /mnt/c/ /mnt/c/d /mnt/c/d/"),
            b"c:/  c:/ c:/d c:/d/"
        );

        env::set_var("WSLGIT_MOUNT_ROOT", "/abc/");
        assert_eq!(
            &*translate_path_to_win(b"/abc/d/some path/a file.md"),
            b"d:/some path/a file.md"
        );
        assert_eq!(
            &*translate_path_to_win(b"origin  /abc/c/path/ (fetch)"),
            b"origin  c:/path/ (fetch)"
        );
        let multiline = b"mirror  /abc/c/other/ (fetch)\nmirror  /abc/c/other/ (push)\n";
        let multiline_result = b"mirror  c:/other/ (fetch)\nmirror  c:/other/ (push)\n";
        assert_eq!(
            &*translate_path_to_win(&multiline[..]),
            &multiline_result[..]
        );
        assert_eq!(
            &*translate_path_to_win(b"/abc/c  /abc/c/ /abc/c/d /abc/c/d/"),
            b"c:/  c:/ c:/d c:/d/"
        );

        env::set_var("WSLGIT_MOUNT_ROOT", "/");
        assert_eq!(
            &*translate_path_to_win(b"/d/some path/a file.md"),
            b"d:/some path/a file.md"
        );
        assert_eq!(
            &*translate_path_to_win(b"origin  /c/path/ (fetch)"),
            b"origin  c:/path/ (fetch)"
        );
        let multiline = b"mirror  /c/other/ (fetch)\nmirror  /c/other/ (push)\n";
        let multiline_result = b"mirror  c:/other/ (fetch)\nmirror  c:/other/ (push)\n";
        assert_eq!(
            &*translate_path_to_win(&multiline[..]),
            &multiline_result[..]
        );
        assert_eq!(
            &*translate_path_to_win(b"/c  /c/ /c/d /c/d/"),
            b"c:/  c:/ c:/d c:/d/"
        );
    }

    #[test]
    fn no_path_translation() {
        env::remove_var("WSLGIT_MOUNT_ROOT");
        assert_eq!(
            &*translate_path_to_win(b"/mnt/other/file.sh /mnt/ab"),
            b"/mnt/other/file.sh /mnt/ab"
        );

        env::set_var("WSLGIT_MOUNT_ROOT", "/abc/");
        assert_eq!(
            &*translate_path_to_win(b"/abc/other/file.sh /abc/ab"),
            b"/abc/other/file.sh /abc/ab"
        );

        env::set_var("WSLGIT_MOUNT_ROOT", "/");
        assert_eq!(
            &*translate_path_to_win(b"/other/file.sh /ab"),
            b"/other/file.sh /ab"
        );
    }

    #[test]
    fn relative_path_translation() {
        env::remove_var("WSLGIT_MOUNT_ROOT");
        assert_eq!(
            translate_path_to_unix(".\\src\\main.rs".to_string()),
            "./src/main.rs"
        );

        env::set_var("WSLGIT_MOUNT_ROOT", "/abc/");
        assert_eq!(
            translate_path_to_unix(".\\src\\main.rs".to_string()),
            "./src/main.rs"
        );

        env::set_var("WSLGIT_MOUNT_ROOT", "/");
        assert_eq!(
            translate_path_to_unix(".\\src\\main.rs".to_string()),
            "./src/main.rs"
        );
    }

    #[test]
    fn arguments_path_translation() {
        env::remove_var("WSLGIT_MOUNT_ROOT");
        assert_eq!(
            translate_path_to_unix("--file=C:\\some\\path.txt".to_owned()),
            "--file=/mnt/c/some/path.txt"
        );

        assert_eq!(
            translate_path_to_unix("-c core.editor=C:\\some\\editor.exe".to_owned()),
            "-c core.editor=/mnt/c/some/editor.exe"
        );

        env::set_var("WSLGIT_MOUNT_ROOT", "/abc/");
        assert_eq!(
            translate_path_to_unix("--file=C:\\some\\path.txt".to_owned()),
            "--file=/abc/c/some/path.txt"
        );

        assert_eq!(
            translate_path_to_unix("-c core.editor=C:\\some\\editor.exe".to_owned()),
            "-c core.editor=/abc/c/some/editor.exe"
        );

        env::set_var("WSLGIT_MOUNT_ROOT", "/");
        assert_eq!(
            translate_path_to_unix("--file=C:\\some\\path.txt".to_owned()),
            "--file=/c/some/path.txt"
        );

        assert_eq!(
            translate_path_to_unix("-c core.editor=C:\\some\\editor.exe".to_owned()),
            "-c core.editor=/c/some/editor.exe"
        );
    }
}
