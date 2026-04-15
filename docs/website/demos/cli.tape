# Context Keeper CLI Demo
# Generates: static/demos/cli-add-search.gif
# Run: vhs demos/cli.tape
# Requires: context-keeper-cli built and in PATH

Output docs/website/static/demos/cli-add-search.gif
Output docs/website/static/demos/cli-add-search.mp4

Set Shell "zsh"
Set FontFamily "JetBrains Mono"
Set FontSize 14
Set Width 900
Set Height 500
Set Padding 24
Set Theme { "name": "ContextKeeper", "black": "#0a0a0b", "red": "#ea580c", "green": "#4ade80", "yellow": "#eab308", "blue": "#3b82f6", "magenta": "#a78bfa", "cyan": "#22d3ee", "white": "#fafafa", "brightBlack": "#71717a", "brightRed": "#fb923c", "brightGreen": "#86efac", "brightYellow": "#fde047", "brightBlue": "#60a5fa", "brightMagenta": "#c4b5fd", "brightCyan": "#67e8f9", "brightWhite": "#ffffff", "background": "#0a0a0b", "foreground": "#fafafa", "cursor": "#fb923c", "selectionBackground": "#27272a" }
Set TypingSpeed 40ms
Set WindowBar Colorful
Set WindowBarSize 28

Sleep 500ms

# Step 1: Add a memory
Type "context-keeper add --text 'Alice is a senior engineer at Acme Corp. She leads the payments team and joined in March 2024.' --source 'onboarding'"
Sleep 300ms
Enter
Sleep 3s

# Step 2: Search for it
Type "context-keeper search --query 'Who works at Acme?'"
Sleep 300ms
Enter
Sleep 3s

# Step 3: Inspect the entity
Type "context-keeper entity --name 'Alice'"
Sleep 300ms
Enter
Sleep 4s

Sleep 2s
