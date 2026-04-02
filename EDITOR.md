## Style:

Stylized gritty orange and dark metallic purples. No native form styles (all
stylized), combo-boxes are fuzzy matched.

## Initial Layout:

If the user has no save file(s) open, they are given the choice to open a
directory or a file. They can drag it into the window as well.

| BL4EDIT           < |
|---------------------|
| PROFILE -- <hints>  |
| VEX -- <hints>      |
| C4SH -- <hints>     |

The initial view if saves are found is a simple stylized list starting with
profile for profile.sav and then followed by each character ordered by their
save file name. 1.sav, 2.sav, etc

The line should have a character icon to the left of it and become tabs on the
left below the header when content is selected / open. These tabs are hidden
if a single file is open since we can't switch between files.

The hints let people know what's in each save. For profiles, it says things
like "bank", letting the user know what's in that option. Character saves say
things about the character. Level, primary chosen skill tree, etc.

The < on the right is a drawer that opens for more tools on the right side. The
drawer will have more advanced tools at the top of it (like managing the
player's local item database) but also has access to a change list in the
bottom of it. The change list is a diff of everything that will change in all
open files if the user saves and lets them verify the changes they want.

## Editor details layout

Most pages are stylized forms organized into sections of similar domains. When
these pages are open, the profile/vex/c4sh selection goes away and a tab bar
appears at the top below the header to allow the user to choose a section. For
instance, if I click cash, the header may be:

/ LOADOUT / BACKPACK / SKILLS / SPECIALIZATIONS / SDUs / STATS / MAP /
