Output assets/ig-demo.gif
Set Shell "bash"
Set FontSize 18
Set Width 1200
Set Height 700
Set Framerate 24
Set Padding 28
Set Theme "TokyoNight"
Env CLICOLOR_FORCE "1"
Env PS1 "~ $ "
# Re-record locally: vhs assets/ig-opencode.tape

Hide
Type "export PATH=/home/bruno/githubworkspace/ivygrep/target/release:$PATH"
Enter
Sleep 300ms
Type "cd /home/bruno/githubworkspace/ivygrep"
Enter
Sleep 300ms
Show

Sleep 500ms
Type "ig 'how does merkle incremental indexing work'"
Sleep 1500ms
Enter
Sleep 3000ms
