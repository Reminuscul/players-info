# Players Info 1.0

Pour [Horizon](https://playhorizon.dev) (Minecraft: Java Edition 26.3 réécrit en Rust). Le mod
affiche les infos du joueur que tu regardes, autour de lui.

C'est un mod natif de type *trait mod* : le launcher l'intègre au jeu en le compilant.

## Autour du joueur visé

Vise un joueur : ses infos apparaissent **autour de lui**, à l'écran, et le suivent quand il bouge.
Elles rapetissent quand il s'éloigne. Il n'y a ni bulle ni panneau : chaque information se place à
côté de ce qu'elle concerne.

```
                    ♥♥♥♥♥♥♥♡♡♡              ← cœurs
           Survie   [ Alex_PvP ]   ▂▄▆█ 42 ms
                        ___
            casque  ▢  |   |  ▢  Épée en diamant
          plastron  ▢  |   |     Tranchant V, Solidité III
                       |   |     Durabilité : 1171 / 1561
         jambières  ▢  |   |  ▢  Totem d'immortalité
                       |   |     Sprint  Mange : Pomme dorée
            bottes  ▢   ‾‾‾      Sur : Cheval
                                 à 4.5 blocs · 0.0 / 64.0 / 4.5
                 Touché par Steve ×2
                 Totem d'immortalité utilisé
```

Les infos n'apparaissent que pour un joueur réellement en vue : la ligne de vue est vérifiée, donc
pas à travers les murs, ni pour un joueur invisible ou en spectateur. Elles restent un instant
quand tu détournes le regard, puis s'effacent en se resserrant contre lui.

### Au-dessus du pseudo

Les **cœurs** du HUD, seuls. Ils se comportent comme les tiens :
- ils clignotent quand le joueur est touché, et les cœurs perdus restent en blanc une seconde ;
- ils tremblent quand il est presque mort ;
- les cœurs d'absorption sont dorés ;
- ils sont gelés dans la neige poudreuse, et hardcore dans un monde hardcore.

Le pseudo lui-même reste celui du jeu, avec ses couleurs d'équipe et ses préfixes : le mod n'y
ajoute rien.

### De part et d'autre du pseudo

Le **mode de jeu** à gauche ; à droite, le **ping** avec les barres de la liste des joueurs et la
latence en ms.

### À gauche du corps

Les **pièces d'armure**, chacune à hauteur de ce qu'elle protège : le casque à la tête, le
plastron au torse, les jambières aux jambes, les bottes aux pieds. De loin, elles se resserrent.
Le jeu les dessine avec l'effet brillant des enchantements et leur barre d'usure.

### À droite du corps

- **L'objet en main** et ses lignes, comme son infobulle : nom dans la couleur de sa rareté
  (italique s'il est renommé), enchantements en gris (malédictions en rouge), `Durabilité`. L'icône
  pulse quand le joueur frappe.
- **L'objet de la seconde main**.
- **Posture et action** : accroupi, sprint, nage, vol en élytres, tourbillon, dort, en feu,
  lumineux ; *Bloque*, *Bande son arc*, *Mange : Pomme dorée*, *Boit : Potion*…
- **Monture** : *Sur : Cheval*, bateau, chameau…
- **Distance**, **position** et vitesse quand il bouge.

### Sous les pieds

Les **événements** des dernières secondes, dans les couleurs du chat. Chacun apparaît en glissant,
puis s'efface :
- coups reçus avec le nom de l'attaquant ; plusieurs coups rapprochés se regroupent
  (*Touché par Steve ×3*) ;
- coups critiques ;
- totem utilisé ;
- coup bloqué au bouclier, bouclier désactivé ;
- monte sur / descend de ;
- changement de mode de jeu ;
- changement d'objet en main ;
- téléportation.

Tout est dessiné par le jeu : sa police, ses sprites, ses icônes d'objets. Ton pack de ressources
s'y applique. Les noms d'objets, d'enchantements et d'entités viennent de ses fichiers de langue.

### Ou dans une infobulle

Avec la disposition « infobulle » (`layout = "panel"`), les mêmes infos sont réunies dans une
infobulle d'objet, placée à droite, à gauche, en haut ou près du viseur. Les objets y sont dans
des emplacements de hotbar.

## Réglages

- **Touche** `I` : activer ou désactiver le mod (modifiable dans Options > Mods > Players Info).
- **En jeu** : Options > Mods > Players Info. Il y a un bouton par réglage : disposition (autour
  du joueur ou infobulle), place de l'infobulle, joueur suivi (sous le viseur ou le plus proche),
  portée, taille, opacité du fond, chaque information et la langue. Chaque clic est
  enregistré.
- **Fichier** : `config/players_info-client.toml`, commenté. On y trouve aussi les décalages de
  l'infobulle en pixels et le délai avant que les infos s'effacent.
  - Une modification est relue en moins d'une seconde.
  - En cas d'erreur, les réglages en cours sont gardés et l'erreur s'affiche dans la barre
    d'action.

## D'où viennent les informations

Rien d'autre que ce que le serveur envoie au client :

| Information | Source |
|---|---|
| Mode de jeu, ping | Liste des joueurs (tab) |
| Santé, absorption, posture, sprint, feu, objet utilisé | Données d'entité |
| Équipement et enchantements | Paquets d'équipement |
| Monture | Paquets de passagers |
| Coups, critiques, totems, boucliers | Paquets d'événements d'entité |

Un serveur qui masque ou falsifie une donnée (santé cachée par un anti-triche, objets sans
enchantements) donne des infos à l'avenant.

La santé maximale n'a pas de nom côté client : c'est celle d'un joueur en vanilla (20), ou la
plus haute vue si elle est plus grande. La durabilité suit les valeurs vanilla.

## Installation

Le mod est épinglé sur la Nightly du 2026-11-05 (`f319701`). Décompresse
`players_info-1.0.0.hnmod` (c'est un zip), puis lance :

```sh
horizon-launcher native-build <dossier décompressé> --instance <ton instance> --trust-native-code
```

Il peut être compilé dans la même instance que d'autres mods épinglés sur ce commit (Lucid, par
exemple).

Licence MIT.
