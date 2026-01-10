# SymLinkManager - Project Context

## Resume rapide
- Projet dans `D:\Medias\Divers\Tools\SymLinkManager\SymLinkManager` (Tauri + Svelte).
- Objectif: scanner, exporter et recreer des symlinks Windows/Linux avec remap de racines.
- UI en anglais, theme PlexTools (fond sombre global, cards sombres, texte clair, accents orange).

## Agent instructions
- See `AGENTS.md` for operational rules and editing guidance.
- If `AGENTS.md` conflicts with this document, follow `AGENTS.md`.

## Decisions UX principales
- Deux onglets: Scan & Export / Import & Recreate.
- Layout aligne en haut (pas de centrage vertical).
- Fond sombre applique a la racine pour couvrir 100% de la hauteur.
- Status global en bas (bulle discrete) visible sur tous les onglets.

## Fonctionnalites principales
- Scan de symlinks dans un dossier racine (OK/Broken).
- Export JSON avec `relative`, `target`, `status`, `link_is_dir`.
- Import JSON + recreation en bulk.
- Root remap rules pour changer les prefixes de `target` (Z:, X:, etc.).
- Elevation auto Windows (UAC) si creation de symlink bloquee.
- Recherche dynamique dans les resultats (multi-termes et exclusions avec `-term`).
- Export respecte le filtre de recherche (si vide, export complet).

## Format JSON
```
{
  "src_root": "D:\\Medias\\Streaming",
  "entries": [
    {
      "relative": "Series\\Show\\link.mkv",
      "target": "Z:\\magnets\\Show\\file.mkv",
      "status": "OK",
      "link_is_dir": true
    }
  ]
}
```

## Root remap rules (comportement)
- Appliquees sur `target` uniquement.
- Premiere regle qui matche gagne.
- Normalisation des slashs et case-insensitive sur Windows.

## Elevation Windows
- Si erreur `os error 1314`, l'app declenche un relaunch admin.
- Le process admin lit un job temporaire et ecrit un resultat temporaire dans `%TEMP%`.

## Fichiers modifies principaux
- `src/routes/+page.svelte`: UI onglets, table scan, remap, status bubble.
- `src-tauri/src/lib.rs`: commandes, symlinks, remap, elevation admin.
- `src-tauri/src/main.rs`: detecte job admin et execute.
- `src-tauri/tauri.conf.json`: bundle inactive (portable), metadata app.

## Build / nettoyage
- Dev: `npm run tauri dev`
- Build portable Windows: `npm run tauri build` (exe dans `src-tauri/target/release/`).
- Dossiers safe a supprimer: `src-tauri/target`, `node_modules`, `.svelte-kit`, `build`.

## Notes
- Sur Windows, symlink = droits admin ou Developer Mode.
- Pas de panels clairs, pas de fond clair.
- Preview import affiche racines detectees + echantillon de liens.
- Dossiers safe a supprimer: `src-tauri/target`, `node_modules`, `.svelte-kit`, `build`.

## Derniere mise a jour
- 2026-01-10

## Changelog
- 2026-01-10: Fenetre Tauri 1380x745 centree; stats top-right avec Entries + Import loaded sur une ligne, separateur, centrage des labels/valeurs (y compris Scan root).
- 2026-01-09: UI onglets + theme PlexTools, import/export, remap roots, elevation admin, preview roots + sample.
- 2026-01-09: Recherche dynamique (multi-termes + exclusions), export filtre, status bubble.


