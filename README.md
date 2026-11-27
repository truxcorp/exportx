# ExportPro

*[English version](README.en.md)*

Trie et renomme les photos et vidéos d'un appareil photo dans :

```
<dossier>/AAAA/MM/JJ/<nom de séance>/p/o/<ext>/     originaux photo (cr2, jpg, xmp, ...)
<dossier>/AAAA/MM/JJ/<nom de séance>/p/s/1/jpg/     vide, pour les photos retouchées
<dossier>/AAAA/MM/JJ/<nom de séance>/v/o/<ext>/     originaux vidéo (mov, mp4, ...)
<dossier>/AAAA/MM/JJ/<nom de séance>/v/s/1/<ext>/   vide, un par type de vidéo trouvé
```

Les fichiers deviennent `<préfixe>_<n>.<ext>`. Les fichiers qui ont le même nom de base
(`IMG_1.CR2` + `IMG_1.JPG`, `MVI_1.MOV` + `MVI_1.THM`) reçoivent le même numéro. La numérotation
suit l'heure de prise de vue. Si le dossier de séance contient déjà des fichiers, la numérotation
reprend à la suite. La date est celle du fichier le plus ancien. Chaque renommage est consigné
dans `exportpro.log`, dans le dossier de séance.

## Utilisation sur Mac

1. Copiez `ExportPro.command` sur le Mac. Si le fichier a perdu son droit d'exécution, lancez
   une fois `chmod +x ExportPro.command`.
2. Double-cliquez dessus. Une fenêtre Terminal s'ouvre, puis des boîtes de dialogue demandent le
   dossier, le nom de la séance et le préfixe des fichiers (`txo` par défaut). Une dernière
   boîte de dialogue demande confirmation.
3. La première fois, macOS peut bloquer le fichier parce qu'il a été téléchargé. Sous macOS 14
   et antérieur, faites un clic droit dessus, choisissez *Ouvrir*, puis cliquez de nouveau sur
   *Ouvrir*. Sous macOS 15 et ultérieur, double-cliquez une fois dessus, puis allez dans
   *Réglages Système > Confidentialité et sécurité* et cliquez sur *Ouvrir quand même*. Vous
   pouvez aussi lancer `xattr -d com.apple.quarantine ExportPro.command` dans Terminal.
4. Si les photos sont sur une carte SD ou dans Bureau/Documents, macOS peut demander si Terminal
   a le droit d'y accéder. Cliquez sur *Autoriser*, sinon le dossier paraîtra vide.

Depuis un terminal, vous pouvez aussi lancer `./ExportPro.command /chemin/vers/dossier`.
Définissez `EXPORTPRO_NO_GUI=1` pour que les questions soient posées dans le terminal plutôt
que dans des boîtes de dialogue.

### zsh et Apple Silicon

zsh est le shell par défaut depuis macOS 10.15, et le seul par défaut sur les Mac M1 à M5. Cela
n'a pas d'importance ici : le script s'exécute toujours dans `/bin/bash` 3.2, que tous les Mac
fournissent encore. Double-clic, `./ExportPro.command`, `zsh ExportPro.command`,
`sh ExportPro.command` et `source ExportPro.command` confient tous le travail à bash. Quand il
est sourcé, le script rend la main à votre shell au lieu de fermer la fenêtre Terminal. Rien ne
dépend du processeur, Rosetta n'est donc pas nécessaire.

## Pourquoi du bash pur

`/bin/bash` 3.2 et `osascript` sont fournis avec toutes les versions de macOS, de 10.5 (2007) à
aujourd'hui, sur Intel comme sur Apple Silicon. Python a été retiré dans macOS 12.3. Un binaire
compilé en C ou en Rust demanderait une compilation universelle, ainsi qu'une signature ou un
contournement de Gatekeeper. Ce script n'utilise que les outils de base (`stat`, `date`, `find`,
`sort`, `awk`, `mv`, `xattr`).

## Tests

```
bash tests/test_exportpro.sh
podman run --rm -v "$PWD":/w:Z -w /w docker.io/library/bash:3.2 bash tests/test_exportpro.sh
```

## Application de bureau (en cours)

Une application multiplateforme (Tauri v2, cœur en Rust) se construit à côté du script :
`docs/plan/tauri-prototype.md`. Contrôles, les mêmes que la CI :

```
make hooks                  # une fois par clone
make lint typecheck test-unit
make ci                     # tout, sécurité comprise, dans l'image du Containerfile (podman)
make box T=bundle           # l'application Linux : target/box/release/bundle/{appimage,deb}
```

Les mêmes fonctions dans un terminal : `exportpro plan|apply --source <carte> --dest <photos> --shoot <nom>`,
puis `exportpro erase <dossier de la séance>` (vers la corbeille) et `exportpro undo <dossier de la séance>`.
