# Players Info

**Pour Minecraft Horizon 26.3.** Vise un joueur et ses infos apparaissent **autour de lui**, à l'écran, avec la police, les sprites et les icônes d'objets du jeu. Elles le suivent quand il bouge et rapetissent quand il s'éloigne.

![Players Info sur un joueur](docs/apercu.png)

- **Au-dessus du pseudo :** la barre de cœurs, dessinée comme celle du jeu (absorption, clignotement aux dégâts, hardcore, gel).
- **À côté du pseudo :** le mode de jeu, et le ping avec les barres et les ms.
- **À gauche du corps :** l'armure, chaque pièce à la hauteur de ce qu'elle couvre.
- **À droite du corps :** l'objet en main (nom, enchantements, durabilité), la posture et l'action (sprint, vol en élytres, tire à l'arc…), la distance, les coordonnées et la vitesse.
- **Sous les pieds :** les derniers événements (touché par…, coup critique, totem, changement d'objet…). Les répétitions se regroupent (`Hit by Okun ×7`).

| En combat | En PvP |
|---|---|
| ![Touché, les cœurs baissent](docs/combat.png) | ![Sur Hypixel](docs/pvp.png) |

Seul un joueur réellement en vue est affiché : pas à travers les murs, et pas les joueurs invisibles. Les spectateurs ne sont visibles que si tu l'es aussi. Le mod n'affiche que ce que le serveur envoie déjà à ton client.

## Installation

1. Télécharge `players_info-1.0.0.hnmod` dans les [Releases](../../releases).
2. Ajoute-le dans le launcher Horizon, qui le compile avec le jeu.

## Réglages

Dans **Options > Mods > Players Info**, tu peux régler :
- chaque élément affiché ;
- le joueur montré : celui sous le viseur ou le plus proche ;
- la portée et la taille.

## Sources

- `bundle/` : le mod tel que le launcher le compile.
- `tools/package.py` : construit le `.hnmod`. À chaque release publiée, GitHub le construit à partir de ces sources et l'ajoute aux fichiers de la release.

## Licence

[MIT](LICENSE) © 2026 Rémi
