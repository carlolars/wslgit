# WSLGit (fork-patch)

This project provides a small executable that forwards all arguments
to `git` running inside Bash on Windows/Windows Subsystem for Linux (WSL).  
This branch (`fork-patch`) is patched to make it possible for [Fork](https://www.fork.dev) to use `wslgit` instead of the `git` that comes bundled with Fork, see instructions [below](#usage-in-fork).

The primary reason for this tool is to make the Git plugin in
Visual Studio Code (VSCode) work with the `git` command installed in WSL.
For these two to interoperate, this tool translates paths
between the Windows (`C:\Foo\Bar`) and Linux (`/mnt/c/Foo/Bar`)
representations.

## Download

The latest binary release can be found on the
[releases page](https://github.com/carlolars/wslgit/releases).

You may also need to install the latest
[*Microsoft Visual C++ Redistributable for Visual Studio 2017*](https://aka.ms/vs/15/release/vc_redist.x64.exe).


## Usage in VSCode

To use this inside VSCode, put the `wslgit.exe` executable somewhere on
your computer and set the appropriate path in your VSCode `settings.json`:

```
{
    "git.path": "C:\\CHANGE\\TO\\PATH\\TO\\wslgit.exe"
}
```

Also make sure that you use an SSH key without password to access your
git repositories, or that your SSH key is added to a SSH agent running
within WSL before starting VSCode.
*You cannot enter your passphrase in VSCode!*

If you use a SSH agent, make sure that it does not print any text
(like e.g. *Agent pid 123*) during startup of an interactive bash shell.
If there is any additional output when your bash shell starts, the VSCode
Git plugin cannot correctly parse the output.


## Usage from the command line

Put the directory containing the executable somewhere on your Windows `Path`
environment variable and optionally rename `wslgit.exe` to `git.exe`.
To change the environment variable, type
`Edit environment variables for your account` into Start menu/Windows search
and use that tool to edit `Path`.

You can then just run any git command from a Windows console
by running `wslgit COMMAND` or `git COMMAND` and it uses the Git version
installed in WSL.


## Usage in Fork
[Fork](https://fork.dev) is a Git GUI tool for Windows (and Mac) that use its own portable version of `Git for Windows`.  
To make Fork use `git from WSL` its original `git.exe` must be replaced with a `wslgit.exe` patched for Fork and, for interactive rebase to work, a wrapper script that calls `Fork.RI.exe` with the arguments converted from Unix paths to Windows paths must be used.

**Instructions**
1. Get the *fork-patch* version of `wslgit.exe`:
   1. Download the latest *fork-patch* binary release from the [releases page](https://github.com/carlolars/wslgit/releases), or
   2. Build the branch `fork-patch`, see build instructions [above](#building-from-source).
2. Rename `wslgit.exe` to just `git.exe` and replace Fork's *git.exe* with the renamed *wslgit.exe*. Fork's git.exe is at the time of writing located in `%HOMEPATH%\AppData\Local\Fork\gitInstance\2.20.1\bin`.
3. Copy the script `Fork.RI` to Fork's application directory, which is something like `%HOMEPATH%\AppData\Local\Fork\app-1.39.0\` depending on the version.
4. From a WSL terminal, make sure that both the `Fork.RI.exe` and the `Fork.RI` script are executable:  
   ```
   $ chmod +x ~/AppData/Local/Fork/app-1.39.0/Fork.RI*
   ```
**Important!** Steps 3 and 4 must be repeated every time Fork is updated. Step 2 needs to be repeated if Fork updates its bundled git.


## Remarks

Currently, the path translation and shell escaping is very limited,
just enough to make it work in VSCode.

All absolute paths are translated, but relative paths are only
translated if they point to existing files or directories.
Otherwise it would be impossible to detect if an
argument is a relative path or just some other string.
VSCode always uses forward slashes for relative paths, so no
translation is necessary in this case.

Additionally, be careful with special characters interpreted by the shell.
Only spaces and newlines in arguments are currently handled.


## Advanced Usage

Per default, `wslgit` executes `git` inside the WSL environment through bash
started in interactive mode. This is to automatically support the common case
where `ssh-agent` or similar tools are setup by `.bashrc` in interactive mode.
However, this may significantly slow down the execution of git commands.
To improve startup time, you can configure `wslgit` to execute git via a
non-interactive bash session. This can be achieved using one of the following
two methods:

  - In Windows, set the environment variable `WSLGIT_USE_INTERACTIVE_SHELL` to
    `false` or `0`. This forces `wslgit` to start bash in non-interactive mode.
  - Alternatively, if the Windows environment variable `BASH_ENV` is set to
    a bash startup script and the environment variable `WSLENV` contains the
    string `"BASH_ENV"`, then `wslgit` assumes that the forced startup script
    from `BASH_ENV` contains everything you need, and therefore also starts
    bash in non-interactive mode.

This feature is only available in Windows 10 builds 17063 and later.

## Mount Root

The default mount root is `/mnt/`, but if it has been changed using `/etc/wsl.conf`
then `wslgit` must be instructed to use the correct mount root by, in Windows,
setting the environment variable `WSLGIT_MOUNT_ROOT` to the new root path.  
If, for example, the mount root defined in wsl.conf is `/` then set `WSLGIT_MOUNT_ROOT` to just `/`.

## Logging

To aid in trouble shooting a simple file log can be enabled by setting the environment variable `WSLGIT_ENABLE_LOGGING` to **`true`** or **`1`**. The logfile, `wslgit.log`, will be stored in the same folder as the `wslgit.exe` executable.

The log stores the input arguments to `wslgit.exe` and the resulting arguments to `wsl.exe`, so be aware that there likely will be information such as your **Windows user name** or **project folders** in the log file.

The performance impact when enabled is small, but the file will continue to grow as long as logging is enabled so it's not recommended to enable logging for more than a limited time.

## Building from source

First, install Rust from https://www.rust-lang.org. Rust on Windows also
requires Visual Studio or the Visual C++ Build Tools for linking.

The final executable can then be build by running

```
cargo build --release
```

inside the root directory of this project. The resulting binary will
be located in `./target/release/`.

Tests **must** be run using one test thread because of race conditions when changing environment variables:
```bash
# Run all tests
cargo test -- --test-threads=1
# Run only unit tests
cargo test test -- --test-threads=1
# Run only integration tests
cargo test integration -- --test-threads=1
```

