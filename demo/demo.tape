Output demo.gif

Set Shell "bash"
Set TypingSpeed 75ms
Set FontSize 22
Set FontFamily "JetBrains Mono"
Set Width 1300
Set Height 650

Env PS1 "> "

Require "mimir"
Require "clear"

Hide
Type "mkdir -p demo && cd demo"
Enter
Type "clear"
Enter
Show

Type "mimir Demo.md"

Sleep 1s

Enter

Sleep 2s

Type "c"

Sleep 500ms

Type "# Mimir"
Enter

Sleep 500ms

Type "Manage your Markdown wiki from the terminal!"

Sleep 1s

Escape

Sleep 2s

Ctrl+Q

Sleep 1s

Hide
Type "cd .. && rm -r demo"
Enter