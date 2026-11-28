# ExportX

*[English version](README.en.md)*

Copie les photos et vidéos d'une carte d'appareil photo dans une arborescence datée, renommées
`<préfixe>_0001.<ext>` dans l'ordre de prise de vue. Ensuite, si vous le demandez, les originaux
partent à la corbeille, ce qui peut être annulé.

Anciennement ExportPro : les séances qu'il a copiées gardent leur `exportpro.log` (lu et
complété), et ses réglages sont repris au premier lancement.

## État du projet

| Partie | État |
|---|---|
| Application de bureau (Tauri v2, cœur en Rust), FR/EN | MVP terminé, finitions en cours |
| Linux : AppImage, `.deb` | construits à chaque merge sur `main` |
| macOS Apple Silicon : `.dmg` | construit à chaque version publiée (et sur demande), non signé par Apple, pas encore essayé sur un Mac |
| Windows, Mac Intel/universel, signature Apple | après le MVP |
| Script bash d'origine (`ExportPro.command`) | conservé, voir la fin de ce fichier |

Plan et décisions : `docs/plan/tauri-prototype.md`, `docs/adr/`.

## Utiliser l'application

1. **Source** : la carte ou le dossier (sélecteur ou glisser-déposer).
2. **Destination** : votre dossier photos (mémorisé).
3. **Nom de la séance**, **préfixe** (`txo` par défaut), et une date si vous voulez. Sans date,
   chaque fichier prend sa date de prise de vue (EXIF, QuickTime). Si les fichiers couvrent
   plusieurs jours, vous choisissez un seul jour, ou un dossier par jour.
4. **Aperçu** de chaque `original → destination`, puis **Copier**. Chaque copie est vérifiée
   (taille + empreinte) et notée dans `exportx.log`, dans le dossier de la séance. Les
   originaux ne sont pas touchés.
5. Ensuite, si vous voulez, **Effacer les originaux** (vers la corbeille). **Annuler** les
   remet à partir des copies.

Si la destination est sur le même disque que les fichiers, l'aperçu propose **Déplacer**
(instantané, sans place en plus) ou **Copier** (les copies prennent autant de place que les
originaux). Un déplacement s'annule d'un clic ou avec Ctrl+Z ; dans un terminal, `apply --move` affiche la
commande qui l'annule (`exportx undo --run <id> <dossier de la séance>`).

Le bouton **i** liste les formats (vidéos, dates lues) ; **⚙** règle la langue et la recherche
de nouvelle version au démarrage (ou tout de suite). Quand il y en a une, un bandeau propose
« Mettre à jour et redémarrer » (AppImage, app Mac : la mise à jour est vérifiée avec la clé de
mise à jour de l'application avant d'être installée) ou, pour un `.deb`, un lien pour la
télécharger. `exportx --help` donne les mêmes listes.

Arborescences (`--preset`) :
- `classique` (par défaut) : `AAAA/MM/JJ/<séance>/p|v/o/<ext>/`
- `lisible` : `…/photo|video/originaux/<ext>/`

Les mêmes fonctions dans un terminal :

```
exportx plan|apply --source <carte> --dest <photos> --shoot <nom> [--prefix p]
                     [--date AAAA-MM-JJ | --split | --no-date] [--preset classique|lisible]
                     [--move]
exportx erase|undo <dossier de la séance>
exportx undo --run <id> <dossier de la séance>
exportx update-check        # une nouvelle version est-elle sortie ?
exportx --version
```

## Publier une version

`make release V=1.2.3` (sur un `main` à jour et propre) crée l'étiquette `v1.2.3` et l'envoie.
Le workflow *release* construit alors l'AppImage et le `.deb` Linux et le `.dmg` Apple Silicon,
signe les fichiers de mise à jour et les publie sur la page
[Releases](https://github.com/truxcorp/exportx/releases) avec `latest.json`, que lit la mise à
jour de l'application (ADR 0007). Une version comme `1.2.3-rc.1` donne une pré-version.

## Récupérer une version construite

Entre deux versions, les versions construites restent sur l'exécution GitHub, dans *Artifacts*.

- **Linux** : Actions → *build* → la dernière exécution sur `main` → `exportx-linux`.
- **macOS (Apple Silicon)** : entre deux versions, seulement sur demande (gratuit : le
  dépôt est public).
  - La lancer : Actions → *build* → *Run workflow* → cocher **macos** → *Run workflow*.
    Ou dans un terminal : `gh workflow run build.yml -f macos=true`.
  - La télécharger : l'artifact `exportx-macos-arm64` de l'exécution, ou
    `gh run download <run-id> -n exportx-macos-arm64`.
  - L'installer : ouvrez le `.dmg` et glissez ExportX dans *Applications*. L'application
    n'est pas signée, donc macOS la bloque la première fois. Ouvrez-la une fois, allez dans
    *Réglages Système > Confidentialité et sécurité*, puis cliquez sur *Ouvrir quand même*.
    Ou lancez `xattr -dr com.apple.quarantine /Applications/ExportX.app`.

## Développement

Les contrôles tournent sur des runners hébergés par GitHub (variable `RUNNER_LABELS`) : Linux
pour chaque PR, macOS pour la construction Mac.

```
make hooks                  # une fois par clone
make lint typecheck test-unit
make ci                     # tout, sécurité comprise, dans l'image du Containerfile (podman)
make box T=bundle           # l'application Linux : target/box/release/bundle/{appimage,deb}
make bundle-macos           # sur un Mac : target/release/bundle/dmg/*.dmg
```

---

## Script d'origine : `ExportPro.command` (bash, macOS)

Le point de départ, toujours conservé et maintenu (`old_for_ref/` garde les versions
précédentes). Il trie et renomme dans :

```
<dossier>/AAAA/MM/JJ/<nom de séance>/p/o/<ext>/     originaux photo (cr2, jpg, xmp, ...)
<dossier>/AAAA/MM/JJ/<nom de séance>/p/s/1/jpg/     vide, pour les photos retouchées
<dossier>/AAAA/MM/JJ/<nom de séance>/v/o/<ext>/     originaux vidéo (mov, mp4, ...)
<dossier>/AAAA/MM/JJ/<nom de séance>/v/s/1/<ext>/   vide, un par type de vidéo trouvé
```

Les fichiers deviennent `<préfixe>_<n>.<ext>`. Ceux qui ont le même nom de base (`IMG_1.CR2` +
`IMG_1.JPG`) reçoivent le même numéro. La numérotation suit l'heure de prise de vue et reprend
à la suite des fichiers déjà présents dans le dossier de séance. La date est celle du fichier le
plus ancien. Chaque renommage est noté dans `exportpro.log`. Contrairement à l'application, le
script **déplace** les fichiers au lieu de les copier.

**Utilisation sur Mac**

1. Copiez `ExportPro.command` sur le Mac. Si besoin, lancez une fois `chmod +x ExportPro.command`.
2. Double-cliquez dessus. Des boîtes de dialogue demandent le dossier, le nom de la séance et le
   préfixe (`txo` par défaut), puis une confirmation.
3. La première fois, macOS peut le bloquer. Sous macOS 14 et antérieur, faites un clic droit
   dessus et choisissez *Ouvrir*. Sous macOS 15 et ultérieur, allez dans *Réglages Système >
   Confidentialité et sécurité > Ouvrir quand même*. Ou lancez
   `xattr -d com.apple.quarantine ExportPro.command`.
4. Pour une carte SD ou Bureau/Documents, autorisez l'accès de Terminal, sinon le dossier paraît
   vide.

Depuis un terminal : `./ExportPro.command /chemin/vers/dossier`. Définissez
`EXPORTPRO_NO_GUI=1` pour que les questions soient posées dans le terminal plutôt que dans des
boîtes de dialogue. Le script tourne toujours dans `/bin/bash` 3.2, fourni par tous les Mac,
Intel ou Apple Silicon, même lancé depuis zsh. Il n'a besoin ni de Python, ni de Rosetta, ni de
signature.

Tests : `bash tests/test_exportpro.sh`, ou en bash 3.2 :
`podman run --rm -v "$PWD":/w:Z -w /w docker.io/library/bash:3.2 bash tests/test_exportpro.sh`
