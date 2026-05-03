# Elden Ring examples

## Debug line
Draws a CSEzDraw debug line from the players position to 1m in front of the player in the direction they're facing.

[Code](debug-line/src/lib.rs).

<details>
<summary>Preview image</summary>

![Debug line rendered by example mode code](img/example-mod-debug-line.png)

</details>

## Apply speffect
Applies and removes an speffect based on keypresses (O to apply and P to remove).

[Code](apply-speffect/src/lib.rs).

<details>
<summary>Preview image</summary>

![Speffect applied on player after pressing keybind](img/example-mod-apply-speffect.png)

</summary>
</details>

## Hot randomizer
Runtime randomizer playground. It currently randomizes the active left/right weapon slot with F1/F2,
and is structured so spell, armor, or other parameter randomizers can be added as separate modules.

[Code](2-hot-randomizer/src/lib.rs).

## Spawn asset (AEG)
Spawns bloodied poop at twice the scale at the players location whenever H is pressed.

[Code](spawn-asset/src/lib.rs).

<details>
<summary>Preview image</summary>

![Asset spawned at players location after pressing keybind](img/example-mod-spawn-asset.png)

</summary>
