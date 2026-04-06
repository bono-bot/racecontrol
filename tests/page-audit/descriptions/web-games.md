# Page: Games
**App:** web  
**URL:** http://192.168.31.23:3200/games  
**Auth:** Required

## Expected Layout
- Left sidebar navigation
- Main content area with game catalog (cards or table)
- Each game entry: game name, icon/image, session count, reliability score
- Possible tabs or sections for different game categories

## Expected Data
- List of available racing games (AC, F1, Forza, iRacing, LMU, etc.)
- Per-game stats: total sessions, reliability score, launch success rate
- **Dynamic content (ignore for layout comparison):** `.game-count`, `.reliability-score`, timestamps, relative times

## Key Interactions
- Clickable game entries for detail/config view
- Links to game reliability and timeline sub-pages
- Possible game enable/disable toggles

## What "Wrong" Looks Like
- Empty game list (API returning no games)
- Unstyled HTML (static files 404)
- Only 1-2 games shown when more exist (incomplete catalog)
- All reliability scores showing 0% or N/A
- Login redirect instead of games page
- Error loading game data
