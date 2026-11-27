// Screen texts in French and English (behaviour 14). Messages computed by the Rust side
// (refusals, errors) arrive already translated.

export const STRINGS = {
  title: { fr: "ExportX", en: "ExportX" },
  lang_switch: { fr: "English", en: "Français" },
  close: { fr: "Fermer", en: "Close" },
  info_title: { fr: "Informations", en: "Information" },
  info_videos: { fr: "Vidéos (dossier vidéo)", en: "Videos (video folder)" },
  info_photos: {
    fr: "Tout autre fichier est copié comme une photo. Les fichiers qui ont le même nom de base (IMG_1.CR2, IMG_1.JPG, IMG_1.XMP) gardent le même numéro et vont ensemble : avec une vidéo (IMG_1.THM à côté de IMG_1.MOV), dans le dossier vidéo. Les fichiers sans extension sont ignorés.",
    en: "Every other file is copied as a photo. Files sharing a base name (IMG_1.CR2, IMG_1.JPG, IMG_1.XMP) keep one number and stay together: with a video (IMG_1.THM next to IMG_1.MOV), in the video folder. Files without an extension are skipped.",
  },
  info_dates: { fr: "Date de prise de vue", en: "Capture date" },
  info_dates_text: {
    fr: "Vérifiée sur : {exts}. Les autres fichiers prennent la date qu'ils contiennent si elle est trouvée, sinon leur date de fichier.",
    en: "Checked on: {exts}. Other files take the date stored in them if found, else their file date.",
  },
  info_log: { fr: "Journal et annulation", en: "Log and undo" },
  info_log_text: {
    fr: "Chaque copie ou déplacement est noté dans exportx.log, dans le dossier de la séance (exportpro.log pour les séances d'ExportPro). Effacer les originaux les met à la corbeille, et Annuler les remet. Un déplacement s'annule d'un clic ou avec Ctrl+Z.",
    en: "Each copy or move is written to exportx.log in the shoot folder (exportpro.log in shoots ExportPro copied). Erasing the originals puts them in the Trash, and Undo brings them back. A move is undone with one click or Ctrl+Z.",
  },
  options_title: { fr: "Options", en: "Options" },
  options_language: { fr: "Langue", en: "Language" },
  options_updates: {
    fr: "Rechercher une nouvelle version au démarrage",
    en: "Check for a new version at startup",
  },
  options_updates_help: {
    fr: "Demande à GitHub si une nouvelle version existe ; actif dès la première version publiée.",
    en: "Asks GitHub whether a new version exists; active from the first published release.",
  },
  back: { fr: "Retour", en: "Back" },
  next: { fr: "Continuer", en: "Continue" },
  choose_folder: { fr: "Choisir un dossier…", en: "Choose a folder…" },
  paste_label: { fr: "Ou collez le chemin du dossier :", en: "Or paste the folder's path:" },
  paste_placeholder: { fr: "/chemin/du/dossier", en: "/path/to/folder" },

  source_title: { fr: "1. Les photos à ranger", en: "1. The photos to sort" },
  source_help: {
    fr: "Glissez le dossier de la carte ici, ou choisissez-le.",
    en: "Drop the card's folder here, or choose it.",
  },
  dest_title: { fr: "2. Où les ranger", en: "2. Where to put them" },
  dest_help: {
    fr: "Le dossier où les séances sont rangées.",
    en: "The folder that holds your shoots.",
  },
  details_title: { fr: "3. La séance", en: "3. The shoot" },
  shoot_label: { fr: "Nom de la séance", en: "Shoot name" },
  prefix_label: { fr: "Préfixe des noms de fichier", en: "File name prefix" },
  prefix_help: { fr: "Exemple : {prefix}_0001.jpg", en: "Example: {prefix}_0001.jpg" },
  date_label: { fr: "Date (facultatif)", en: "Date (optional)" },
  date_placeholder: { fr: "AAAA-MM-JJ", en: "YYYY-MM-DD" },
  invalid_date: {
    fr: "Cette date n'existe pas : écrivez-la AAAA-MM-JJ, par exemple 2024-03-05.",
    en: "This date doesn't exist: write it YYYY-MM-DD, e.g. 2024-03-05.",
  },
  date_help: {
    fr: "Vide : la date de prise de vue des fichiers.",
    en: "Empty: the files' capture date.",
  },
  preset_label: { fr: "Arborescence", en: "Folder layout" },
  preset_classique: { fr: "Classique (p/o/cr2…)", en: "Classic (p/o/cr2…)" },
  preset_lisible: { fr: "Lisible (photo/originaux/cr2…)", en: "Readable (photo/originaux/cr2…)" },
  planning: { fr: "Préparation…", en: "Preparing…" },

  days_title: { fr: "Les fichiers couvrent plusieurs jours", en: "The files span several days" },
  day_count: { fr: "{day} : {n} fichier(s)", en: "{day}: {n} file(s)" },
  day_one: { fr: "Tout dans le dossier du {day}", en: "All in the folder of {day}" },
  day_each: { fr: "Un dossier par jour", en: "One folder per day" },
  day_none: { fr: "Sans date", en: "No date" },

  preview_title: { fr: "4. Vérifiez avant de copier", en: "4. Check before copying" },
  preview_count: { fr: "{n} fichier(s) seront copiés dans {dest}", en: "{n} file(s) will be copied to {dest}" },
  preview_safe: {
    fr: "Les originaux ne sont pas touchés.",
    en: "The originals are not touched.",
  },
  preview_title_move: { fr: "4. Vérifiez avant de déplacer", en: "4. Check before moving" },
  preview_count_move: {
    fr: "{n} fichier(s) seront déplacés dans {dest}",
    en: "{n} file(s) will be moved to {dest}",
  },
  mode_same_disk: {
    fr: "La destination est sur le même disque que les fichiers :",
    en: "The destination is on the same disk as the files:",
  },
  mode_move: { fr: "Déplacer", en: "Move" },
  mode_move_help: {
    fr: "instantané, sans place en plus, et vous pourrez annuler.",
    en: "instant, no extra space, and you can undo it.",
  },
  mode_copy: { fr: "Copier", en: "Copy" },
  mode_copy_help: {
    fr: "les copies prennent {size} de plus sur ce disque ; les originaux restent.",
    en: "the copies take {size} more on this disk; the originals stay.",
  },
  mode_copy_no_room: {
    fr: "pas assez de place sur ce disque pour copier {size}.",
    en: "not enough room on this disk to copy {size}.",
  },
  copy: { fr: "Copier", en: "Copy" },
  move: { fr: "Déplacer", en: "Move" },
  moving: { fr: "Déplacement en cours…", en: "Moving…" },
  copying: { fr: "Copie en cours…", en: "Copying…" },

  result_title: { fr: "Terminé", en: "Done" },
  copied: { fr: "{n} fichier(s) copié(s)", en: "{n} file(s) copied" },
  moved: { fr: "{n} fichier(s) déplacé(s)", en: "{n} file(s) moved" },
  undo_move: { fr: "Annuler le déplacement", en: "Undo the move" },
  undo_hint: { fr: "ou {keys}", en: "or {keys}" },
  moved_back: { fr: "{n} fichier(s) remis à leur place", en: "{n} file(s) moved back" },
  failed: { fr: "{n} en échec :", en: "{n} failed:" },
  open_folder: { fr: "Ouvrir le dossier", en: "Open folder" },
  erase_question: {
    fr: "Effacer les originaux ? Ils iront à la corbeille, et vous pourrez annuler.",
    en: "Erase the originals? They go to the Trash, and you can undo it.",
  },
  erase_yes: { fr: "Effacer les originaux", en: "Erase the originals" },
  erase_no: { fr: "Garder les originaux", en: "Keep the originals" },
  erasing: { fr: "Mise à la corbeille…", en: "Moving to the Trash…" },
  erased: { fr: "{n} original(aux) mis à la corbeille", en: "{n} original(s) moved to the Trash" },
  kept: { fr: "{n} gardé(s) :", en: "{n} kept:" },
  undo: { fr: "Annuler l'effacement", en: "Undo erase" },
  undoing: { fr: "Restauration…", en: "Restoring…" },
  moving_back: { fr: "Remise en place…", en: "Moving back…" },
  restored: { fr: "{n} original(aux) restauré(s)", en: "{n} original(s) restored" },
  restart: { fr: "Nouvel export", en: "New export" },
};

/** The text `key` in `lang` (English if unknown), with `{name}` placeholders filled. */
export function t(lang, key, vars = {}) {
  const entry = STRINGS[key];
  if (!entry) return key;
  const text = entry[lang] ?? entry.en;
  return text.replace(/\{(\w+)\}/g, (m, name) => (name in vars ? String(vars[name]) : m));
}
