Output assets/ivygrep-opencode.gif
Set Shell "bash"
Set FontSize 20
Set Width 1280
Set Height 820
Set Framerate 30
Set Padding 28
Set Theme "TokyoNight"
Env CLICOLOR_FORCE "1"
Env PS1 "~ %# "
# Re-record locally: vhs assets/ivygrep-opencode.tape

Hide
Type "cd ~/githubworkspace/opencode"
Enter
Sleep 300ms
Show
Type "ivygrep 'authentication for MCPs' | less -R"
Enter
Sleep 2000ms
Type " "
Sleep 1000ms
Type "q"
Enter
Sleep 500ms
