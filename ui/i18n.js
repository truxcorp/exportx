// Screen texts in French and English (behaviour 14). Messages computed by the Rust side
// (refusals, errors) arrive already translated.

export const STRINGS = {
  title: { fr: "ExportPro", en: "ExportPro" },
  lang_switch: { fr: "English", en: "Français" },
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
  copy: { fr: "Copier", en: "Copy" },
  copying: { fr: "Copie en cours…", en: "Copying…" },

  result_title: { fr: "Terminé", en: "Done" },
  copied: { fr: "{n} fichier(s) copié(s)", en: "{n} file(s) copied" },
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
