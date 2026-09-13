use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LaunchCommand {
    pub program: OsString,
    pub args: Vec<OsString>,
}

impl LaunchCommand {
    pub(crate) fn status(&self) -> std::io::Result<ExitStatus> {
        Command::new(&self.program).args(&self.args).status()
    }

    /// Start the program without waiting for it and return its pid. A
    /// background thread reaps the child, so long-lived callers (the daemon's
    /// `/api/open`) do not leave a zombie process per launch.
    pub(crate) fn spawn_detached(&self) -> std::io::Result<u32> {
        let mut child = Command::new(&self.program)
            .args(&self.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let pid = child.id();
        if let Err(error) = std::thread::Builder::new()
            .name("ig-launch-reaper".to_string())
            .spawn(move || {
                let _ = child.wait();
            })
        {
            tracing::warn!("could not start a reaper for launched process {pid}: {error}");
        }
        Ok(pid)
    }
}

pub(crate) fn tui_editor_launch(
    path: &Path,
    line: usize,
    column: Option<usize>,
) -> Result<LaunchCommand> {
    for name in ["EDITOR", "VISUAL"] {
        if let Some(value) = nonempty_env(name) {
            return editor_launch_from_command(&value, path, line, column)
                .with_context(|| format!("invalid {name} editor command"));
        }
    }

    editor_launch_from_parts([OsString::from(default_tui_editor())], path, line, column)
}

pub(crate) fn web_editor_launch(
    path: &Path,
    line: usize,
    column: Option<usize>,
) -> Result<LaunchCommand> {
    for name in ["IVYGREP_WEB_EDITOR", "IVYGREP_EDITOR", "EDITOR", "VISUAL"] {
        let Some(value) = nonempty_env(name) else {
            continue;
        };
        let launch = editor_launch_from_command(&value, path, line, column)
            .with_context(|| format!("invalid {name} editor command"))?;
        if matches!(name, "EDITOR" | "VISUAL") && is_terminal_editor(&launch.program) {
            continue;
        }
        return Ok(launch);
    }

    for candidate in [
        "code",
        "code-insiders",
        "cursor",
        "codium",
        "windsurf",
        "zed",
        "subl",
    ] {
        if let Some(program) = find_program(candidate) {
            return editor_launch_from_parts([program], path, line, column);
        }
    }

    fallback_file_launch(path)
}

fn nonempty_env(name: &str) -> Option<OsString> {
    std::env::var_os(name).filter(|value| !value.to_string_lossy().trim().is_empty())
}

fn editor_launch_from_command(
    value: &OsStr,
    path: &Path,
    line: usize,
    column: Option<usize>,
) -> Result<LaunchCommand> {
    editor_launch_from_parts(parse_editor_command(value)?, path, line, column)
}

fn editor_launch_from_parts<I>(
    parts: I,
    path: &Path,
    line: usize,
    column: Option<usize>,
) -> Result<LaunchCommand>
where
    I: IntoIterator<Item = OsString>,
{
    let mut parts = parts.into_iter();
    let Some(program) = parts.next().filter(|program| !program.is_empty()) else {
        bail!("editor command must include an executable");
    };
    reject_unsafe_program(&program)?;

    let mut args = parts.collect::<Vec<_>>();
    let line = line.max(1);
    let column = column.map(|column| column.max(1));
    match program_basename(&program).as_str() {
        "code" | "code-insiders" | "codium" | "cursor" | "windsurf" => {
            args.push(OsString::from("-g"));
            args.push(path_with_location(path, line, column, ':'));
        }
        "zed" | "subl" | "mate" | "hx" | "helix" => {
            args.push(path_with_location(path, line, column, ':'));
        }
        "vim" | "nvim" | "vi" => {
            args.push(match column {
                Some(column) => OsString::from(format!("+call cursor({line},{column})")),
                None => OsString::from(format!("+{line}")),
            });
            args.push(path.as_os_str().to_owned());
        }
        "nano" => {
            args.push(match column {
                Some(column) => OsString::from(format!("+{line},{column}")),
                None => OsString::from(format!("+{line}")),
            });
            args.push(path.as_os_str().to_owned());
        }
        "emacs" | "emacsclient" => {
            args.push(match column {
                Some(column) => OsString::from(format!("+{line}:{column}")),
                None => OsString::from(format!("+{line}")),
            });
            args.push(path.as_os_str().to_owned());
        }
        _ => args.push(path.as_os_str().to_owned()),
    }

    Ok(LaunchCommand { program, args })
}

fn path_with_location(
    path: &Path,
    line: usize,
    column: Option<usize>,
    separator: char,
) -> OsString {
    let mut target = path.as_os_str().to_owned();
    target.push(format!("{separator}{line}"));
    if let Some(column) = column {
        target.push(format!("{separator}{column}"));
    }
    target
}

fn reject_unsafe_program(program: &OsStr) -> Result<()> {
    let basename = program_basename_with_extension(program);
    let stem = basename.strip_suffix(".exe").unwrap_or(&basename);
    let is_command_wrapper = matches!(
        stem,
        "cmd"
            | "command.com"
            | "powershell"
            | "pwsh"
            | "sh"
            | "bash"
            | "dash"
            | "zsh"
            | "fish"
            | "ksh"
            | "csh"
            | "tcsh"
    );
    let is_script = [".bat", ".cmd", ".ps1"]
        .iter()
        .any(|extension| basename.ends_with(extension));
    if is_command_wrapper || is_script {
        bail!(
            "unsafe editor program {:?}; configure editor executable directly, without command shells or .cmd/.bat scripts",
            program
        );
    }
    Ok(())
}

fn program_basename(program: &OsStr) -> String {
    program_basename_with_extension(program)
        .trim_end_matches(".exe")
        .to_string()
}

fn program_basename_with_extension(program: &OsStr) -> String {
    Path::new(program)
        .file_name()
        .unwrap_or(program)
        .to_string_lossy()
        .to_ascii_lowercase()
        .trim_end_matches([' ', '.'])
        .to_string()
}

fn is_terminal_editor(program: &OsStr) -> bool {
    matches!(
        program_basename(program).as_str(),
        "vim" | "nvim" | "vi" | "nano" | "emacs" | "emacsclient" | "hx" | "helix"
    )
}

fn find_program(name: &str) -> Option<OsString> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate.into_os_string());
        }
        #[cfg(windows)]
        {
            let candidate = dir.join(format!("{name}.exe"));
            if candidate.is_file() {
                return Some(candidate.into_os_string());
            }
        }
    }
    None
}

#[cfg(windows)]
fn default_tui_editor() -> &'static str {
    "notepad.exe"
}

#[cfg(not(windows))]
fn default_tui_editor() -> &'static str {
    "vim"
}

#[cfg(target_os = "windows")]
fn fallback_file_launch(path: &Path) -> Result<LaunchCommand> {
    editor_launch_from_parts([OsString::from("notepad.exe")], path, 1, None)
}

#[cfg(target_os = "macos")]
fn fallback_file_launch(path: &Path) -> Result<LaunchCommand> {
    Ok(LaunchCommand {
        program: OsString::from("open"),
        args: vec![path.as_os_str().to_owned()],
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn fallback_file_launch(path: &Path) -> Result<LaunchCommand> {
    Ok(LaunchCommand {
        program: OsString::from("xdg-open"),
        args: vec![path.as_os_str().to_owned()],
    })
}

#[cfg(not(windows))]
fn parse_editor_command(value: &OsStr) -> Result<Vec<OsString>> {
    let value = value
        .to_str()
        .context("editor command is not valid Unicode")?;
    let parts = shlex::split(value).context("editor command has invalid quoting")?;
    if parts.is_empty() {
        bail!("editor command must include an executable");
    }
    Ok(parts.into_iter().map(OsString::from).collect())
}

#[cfg(windows)]
fn parse_editor_command(value: &OsStr) -> Result<Vec<OsString>> {
    use std::os::windows::ffi::OsStringExt;

    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::UI::Shell::CommandLineToArgvW;

    let command_line = wide_nul(value)?;
    let mut argc = 0;
    // SAFETY: command_line is NUL-terminated and remains alive until argv is copied.
    let argv = unsafe { CommandLineToArgvW(command_line.as_ptr(), &mut argc) };
    if argv.is_null() {
        return Err(std::io::Error::last_os_error()).context("could not parse editor command");
    }

    let mut parts = Vec::with_capacity(argc.max(0) as usize);
    // SAFETY: successful CommandLineToArgvW returns argc NUL-terminated strings.
    for index in 0..argc.max(0) as usize {
        let argument = unsafe { *argv.add(index) };
        let mut len = 0;
        while unsafe { *argument.add(len) } != 0 {
            len += 1;
        }
        let value = unsafe { std::slice::from_raw_parts(argument, len) };
        parts.push(OsString::from_wide(value));
    }
    // SAFETY: CommandLineToArgvW allocates argv with LocalAlloc.
    unsafe {
        LocalFree(argv.cast());
    }

    if parts.is_empty() || parts[0].is_empty() {
        bail!("editor command must include an executable");
    }
    Ok(parts)
}

#[cfg(windows)]
fn wide_nul(value: &OsStr) -> Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;

    let mut value = value.encode_wide().collect::<Vec<_>>();
    if value.contains(&0) {
        bail!("launcher value contains a NUL character");
    }
    value.push(0);
    Ok(value)
}

/// Age after which a later `open_browser` call removes a redirect file. A cold
/// browser start can take a while to read it, and the file is owner-only.
const BROWSER_REDIRECT_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(120);
const BROWSER_REDIRECT_PREFIX: &str = "open-";

/// Open the Web UI `url` in the system browser without putting it on a command
/// line.
///
/// The URL carries the session token, and process arguments are visible to
/// other local users (`/proc/<pid>/cmdline`, `ps`), both the opener's and the
/// browser's. Like Jupyter, write an owner-only HTML redirect under the ivygrep
/// app home and pass only that file's location to the opener.
pub(crate) fn open_browser(url: &str) -> Result<()> {
    if std::env::var_os("IVYGREP_NO_BROWSER").is_some() {
        return Ok(());
    }
    let redirect = write_browser_redirect(&crate::config::app_home()?.join("browser"), url)?;
    launch_browser(&redirect)
}

/// ShellExecuteW hands the redirect path to the `.html` handler, so the
/// browser's command line carries the file path, not the token.
#[cfg(windows)]
fn launch_browser(redirect: &Path) -> Result<()> {
    use std::ptr;

    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let operation = wide_nul(OsStr::new("open"))?;
    let target = wide_nul(redirect.as_os_str())?;
    // SAFETY: all supplied strings are NUL-terminated and live through the call.
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            operation.as_ptr(),
            target.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        bail!(
            "Windows could not open the browser (ShellExecuteW code {})",
            result as isize
        );
    }
    Ok(())
}

#[cfg(unix)]
fn launch_browser(redirect: &Path) -> Result<()> {
    browser_launch(redirect)
        .spawn_detached()
        .context("failed to launch system browser")?;
    Ok(())
}

/// `open` (macOS) or `xdg-open` for the redirect file's `file://` URL.
#[cfg(unix)]
fn browser_launch(redirect: &Path) -> LaunchCommand {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    LaunchCommand {
        program: OsString::from(program),
        args: vec![OsString::from(file_url(redirect))],
    }
}

/// `file://` URL for an absolute path, percent-encoding every byte outside the
/// unreserved set and `/`.
#[cfg(unix)]
fn file_url(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;

    let mut url = String::from("file://");
    for &byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            url.push(char::from(byte));
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    url
}

/// Write an owner-only page under `dir` that forwards the browser to `url`,
/// removing redirect files left by earlier launches.
fn write_browser_redirect(dir: &Path, url: &str) -> Result<std::path::PathBuf> {
    use std::io::Write as _;

    let dir =
        std::path::absolute(dir).with_context(|| format!("could not resolve {}", dir.display()))?;
    create_private_dir(&dir)?;
    remove_stale_browser_redirects(&dir, BROWSER_REDIRECT_MAX_AGE);
    let path = dir.join(format!(
        "{BROWSER_REDIRECT_PREFIX}{}.html",
        uuid::Uuid::new_v4().simple()
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .with_context(|| format!("could not create browser redirect {}", path.display()))?;
    if let Err(error) = file.write_all(browser_redirect_html(url).as_bytes()) {
        drop(file);
        let _ = std::fs::remove_file(&path);
        return Err(error)
            .with_context(|| format!("could not write browser redirect {}", path.display()));
    }
    Ok(path)
}

/// Create `dir` to hold secrets. On Unix it must end up a mode-0700 directory
/// owned by the current user, never a symlink or another user's directory.
/// Windows files inherit the app home's ACL (the user profile by default).
fn create_private_dir(dir: &Path) -> Result<()> {
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }
    #[cfg(unix)]
    let created = {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).create(dir)
    };
    #[cfg(not(unix))]
    let created = std::fs::DirBuilder::new().create(dir);
    match created {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(error).with_context(|| format!("could not create {}", dir.display()));
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let metadata = std::fs::symlink_metadata(dir)
            .with_context(|| format!("could not inspect {}", dir.display()))?;
        // SAFETY: geteuid has no preconditions and cannot fail.
        let euid = unsafe { libc::geteuid() };
        if !metadata.is_dir() || metadata.uid() != euid {
            bail!(
                "refusing to write the browser redirect into {}: not a directory owned by the current user",
                dir.display()
            );
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
                .with_context(|| format!("could not restrict {} to mode 0700", dir.display()))?;
        }
    }
    Ok(())
}

fn remove_stale_browser_redirects(dir: &Path, max_age: std::time::Duration) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with(BROWSER_REDIRECT_PREFIX) && name.ends_with(".html")) {
            continue;
        }
        let stale = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age >= max_age);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Static page forwarding to `url` through a meta refresh, with no script.
fn browser_redirect_html(url: &str) -> String {
    let url = crate::web::escape_html_attribute(url);
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"referrer\" content=\"no-referrer\">\n<meta http-equiv=\"refresh\" content=\"0;url={url}\">\n<title>Opening ivygrep web</title>\n</head>\n<body>\n<p><a href=\"{url}\">Open ivygrep web</a></p>\n</body>\n</html>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn detached_launch_is_reaped_after_exit() {
        let launch = LaunchCommand {
            program: OsString::from("true"),
            args: Vec::new(),
        };
        let pid = launch.spawn_detached().unwrap();
        let proc_entry = format!("/proc/{pid}");
        let started = std::time::Instant::now();
        while Path::new(&proc_entry).exists() {
            assert!(
                started.elapsed() < std::time::Duration::from_secs(10),
                "detached child {pid} was never reaped"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    #[cfg(unix)]
    #[test]
    fn browser_opener_argv_carries_redirect_file_not_token() {
        use std::os::unix::fs::PermissionsExt;

        let home = tempfile::tempdir().unwrap();
        let token = "test-session-token";
        let url = format!("http://127.0.0.1:4747/?q=auth%20flow&workspace=/repo&token={token}");
        let redirect = write_browser_redirect(&home.path().join("browser"), &url).unwrap();
        let launch = browser_launch(&redirect);

        let argv = std::iter::once(&launch.program)
            .chain(&launch.args)
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(
            argv.iter().all(|argument| !argument.contains(token)),
            "opener argv exposes the session token: {argv:?}"
        );
        assert_eq!(launch.args, vec![OsString::from(file_url(&redirect))]);

        let html = std::fs::read_to_string(&redirect).unwrap();
        assert!(
            html.contains(&format!(
                "content=\"0;url=http://127.0.0.1:4747/?q=auth%20flow&amp;workspace=/repo&amp;token={token}\""
            )),
            "{html}"
        );
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&redirect), 0o600);
        assert_eq!(mode(redirect.parent().unwrap()), 0o700);
    }

    #[cfg(unix)]
    #[test]
    fn redirect_file_url_percent_encodes_path_bytes() {
        assert_eq!(
            file_url(Path::new("/home/me/Ivy Grep/open-1.html")),
            "file:///home/me/Ivy%20Grep/open-1.html"
        );
    }

    #[test]
    fn earlier_browser_redirects_are_removed_after_grace_period() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join("browser");
        let stale = write_browser_redirect(&dir, "http://127.0.0.1:1/?token=a").unwrap();
        let expired = std::time::SystemTime::now()
            - BROWSER_REDIRECT_MAX_AGE
            - std::time::Duration::from_secs(1);
        std::fs::File::options()
            .write(true)
            .open(&stale)
            .unwrap()
            .set_modified(expired)
            .unwrap();
        let recent = write_browser_redirect(&dir, "http://127.0.0.1:1/?token=b").unwrap();
        assert!(!stale.exists(), "expired redirect was kept");

        write_browser_redirect(&dir, "http://127.0.0.1:1/?token=c").unwrap();
        assert!(
            recent.exists(),
            "redirect inside its grace period was removed"
        );
    }

    #[test]
    fn quoted_editor_path_and_prefix_arguments_are_preserved() {
        let command = if cfg!(windows) {
            r#""C:\Program Files\Editor\code.exe" --wait --profile "Ivy Grep""#
        } else {
            r#""/opt/Visual Studio Code/code" --wait --profile "Ivy Grep""#
        };
        let expected_program = if cfg!(windows) {
            r"C:\Program Files\Editor\code.exe"
        } else {
            "/opt/Visual Studio Code/code"
        };

        let launch = editor_launch_from_command(
            OsStr::new(command),
            Path::new("/repo/quoted file.rs"),
            7,
            Some(3),
        )
        .unwrap();

        assert_eq!(launch.program, OsString::from(expected_program));
        assert_eq!(
            launch.args,
            strings(&[
                "--wait",
                "--profile",
                "Ivy Grep",
                "-g",
                "/repo/quoted file.rs:7:3"
            ])
        );
    }

    #[test]
    fn hostile_unicode_path_is_one_opaque_editor_argument() {
        let path = Path::new("/repo/Ω & calc; $(touch nope) \"quote\".rs");
        let launch =
            editor_launch_from_parts(strings(&["cursor", "--reuse-window"]), path, 42, None)
                .unwrap();

        assert_eq!(
            launch.args,
            strings(&[
                "--reuse-window",
                "-g",
                "/repo/Ω & calc; $(touch nope) \"quote\".rs:42"
            ])
        );
    }

    #[test]
    fn editor_location_arguments_follow_editor_conventions() {
        let path = Path::new("src/lib.rs");
        let cases = [
            ("code", strings(&["-g", "src/lib.rs:8:4"])),
            ("zed", strings(&["src/lib.rs:8:4"])),
            ("subl", strings(&["src/lib.rs:8:4"])),
            ("mate", strings(&["src/lib.rs:8:4"])),
            ("hx", strings(&["src/lib.rs:8:4"])),
            ("vim", strings(&["+call cursor(8,4)", "src/lib.rs"])),
            ("nano", strings(&["+8,4", "src/lib.rs"])),
            ("emacs", strings(&["+8:4", "src/lib.rs"])),
        ];

        for (program, expected) in cases {
            let launch =
                editor_launch_from_parts([OsString::from(program)], path, 8, Some(4)).unwrap();
            assert_eq!(launch.args, expected, "{program}");
        }

        for (program, expected) in [
            ("vim", strings(&["+8", "src/lib.rs"])),
            ("nano", strings(&["+8", "src/lib.rs"])),
            ("emacs", strings(&["+8", "src/lib.rs"])),
        ] {
            let launch =
                editor_launch_from_parts([OsString::from(program)], path, 8, None).unwrap();
            assert_eq!(launch.args, expected, "{program}");
        }
    }

    #[test]
    fn unsafe_command_wrappers_and_scripts_are_rejected() {
        for command in [
            "cmd.exe /C code",
            "cmd.exe. /C code",
            "command.com /C code",
            "powershell.exe -Command code",
            "pwsh -Command code",
            "sh -c code",
            "editor.cmd --wait",
            "editor.cmd. --wait",
            "editor.BAT --wait",
        ] {
            let error = editor_launch_from_command(
                OsStr::new(command),
                Path::new("repo & hostile.rs"),
                1,
                None,
            )
            .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("configure editor executable directly"),
                "{command}: {error:#}"
            );
        }
    }

    #[test]
    fn terminal_editor_classification_preserves_web_filtering() {
        assert!(is_terminal_editor(OsStr::new("vim")));
        assert!(is_terminal_editor(OsStr::new("/tools/hx.exe")));
        assert!(!is_terminal_editor(OsStr::new("code")));
    }

    #[cfg(windows)]
    #[test]
    fn windows_browser_url_encoding_is_exact() {
        use std::os::windows::ffi::OsStringExt;

        let url = r#"http://127.0.0.1:7890/?q=a b&workspace=C:\repo\Ω&quoted="yes""#;
        let wide = wide_nul(OsStr::new(url)).unwrap();
        assert_eq!(OsString::from_wide(&wide[..wide.len() - 1]), url);
        assert_eq!(wide.last(), Some(&0));
    }

    #[cfg(windows)]
    #[test]
    fn windows_file_fallback_is_native_notepad() {
        let path = Path::new(r"C:\repo\hostile & name.rs");
        let launch = fallback_file_launch(path).unwrap();
        assert_eq!(launch.program, OsString::from("notepad.exe"));
        assert_eq!(launch.args, vec![path.as_os_str().to_owned()]);
    }

    #[cfg(not(windows))]
    #[test]
    fn invalid_editor_quoting_is_rejected() {
        let error = editor_launch_from_command(
            OsStr::new(r#"code --profile "unterminated"#),
            Path::new("src/lib.rs"),
            1,
            None,
        )
        .unwrap_err();
        assert!(error.to_string().contains("invalid quoting"));
    }
}
