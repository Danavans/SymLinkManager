# SymLink Manager — audit et refonte

Date : 17 septembre 2026.

Périmètre : refonte et optimisations de **Symlink Manager 1.1.0**. Les mesures ci-dessous documentent les vérifications de la refonte ; la mise à jour documentaire et des métadonnées de version ne constitue pas une nouvelle compilation ni une validation native supplémentaire.

## Livré

Interface entièrement réorganisée autour d'un espace de travail graphite/indigo : navigation latérale, inventaire, indicateurs de santé cliquables, recherche documentée, pagination de 100 lignes, états vides, durée du scan et journal d'état lisible. L'import suit quatre étapes : snapshot, destination, règles de remappage et revue des connexions ; l'aperçu reste automatique. Les dialogues natifs HTML gèrent le focus et Échap. Petites fenêtres et réduction des animations sont prises en charge. Tous les textes de l'application restent en anglais.

Le logo SVG représente deux connexions et une flèche de transfert. Le même dessin alimente le logo de navigation, le favicon, les PNG/ICO/ICNS, l'icône de fenêtre et les variantes de plateformes déjà présentes. Aucune police distante, dépendance d'exécution supplémentaire ou nouvelle étape de build.

## Inspection et corrections

| Zone | Constat initial | Traitement |
| --- | --- | --- |
| Scan Rust | Deux `metadata` par cible accessible, validations séquentielles, commande synchrone sur le thread principal | Une lecture réutilisée pour état/type, huit workers bornés, `spawn_blocking` |
| Affichage | Jusqu'à 3 000 lignes DOM, normalisation des clés dans chaque comparaison | 100 lignes/page, résultats dérivés en cache, clés de tri calculées une fois |
| Import preview | Envoi de toutes les lignes alors que l'écran ne montrait essentiellement qu'un exemple par racine | Échantillon demandé de 20, limite backend 100, racines toujours complètes |
| Preview asynchrone | Une réponse obsolète pouvait apparaître pendant le délai du prochain aperçu ; erreurs silencieuses | Invalidation immédiate, état de chargement et erreurs visibles |
| Remplacement | Suppression de l'original avant création : un refus de permission pouvait détruire un fichier | Renommage temporaire de l'original, restauration si création échoue ; dossiers non vides préservés |
| Import JSON | Chemins absolus, `..`, doublons et certains noms spéciaux non contrôlés | Validation commune à chargement, aperçu, conflits et recréation, y compris l'exécuteur élevé |
| Destination | Un parent déjà lié pouvait rediriger les écritures | Refus de traverser un parent symlink ; contrôle juste avant création |
| Chemins | Préfixe UNC étendu supprimé incorrectement ; remappage `/` et longueurs Unicode problématiques | Normalisation UNC, comparaison des composants et tests de limites |
| Statuts | Toute erreur hors permission assimilée à une cible absente | `NotFound` devient Broken ; autres erreurs deviennent Unreadable |
| Élévation | Deux commandes similaires, erreurs du worker perdues, résultat potentiellement lu pendant son écriture | Commande inutilisée supprimée, détails d'erreur transmis, publication du résultat par renommage, IDs validés, job créé sans écrasement |
| Retours utilisateur | Pas de dialogue de résultat pour la recréation non élevée, formulaires modifiables pendant confirmation | Résultat cohérent, erreurs partielles visibles, formulaire verrouillé pendant l'opération |
| Règles incomplètes | Préfixe rempli d'un seul côté ignoré silencieusement | Avertissement et recréation désactivée jusqu'à correction |
| Surface applicative | Police Google distante, plugin opener inutilisé, CSP absente | Police système, plugin/capacité supprimés, CSP locale |
| Organisation | Logique de remappage dupliquée, absence de tests | Helper commun, tests Rust séparés, fixture UI et benchmark reproductible |

Les formats JSON existants, la recherche OR par virgules et exclusions, l'export filtré, les types fichier/dossier, le remappage implicite des cibles internes, la priorité de la première règle et le parcours UAC restent conservés. Les exports contenant une cible `<unreadable>` restent chargeables ; ces entrées échouent explicitement à la recréation plutôt que de créer un faux lien.

## Mesures

Fixture locale Windows : **3 000 jonctions NTFS**, 2 700 cibles accessibles et 300 cibles manquantes, sans parcours des dossiers liés. Sept passages, premier passage écarté, ordre ancien/nouveau alterné ; médiane des six suivants. Build Rust de test non optimisée. L'ancienne fonction est conservée uniquement sous `cfg(test)` dans `scan_baseline.rs`. Chaque passage compare chemin, cible, état et type de toutes les entrées.

| Mesure finale | Ancien | Nouveau |
| --- | ---: | ---: |
| Scan filesystem complet | 175,85 ms | 73,71 ms |
| Lignes DOM, 3 000 résultats | 3 000 | 100 |
| Lectures metadata d'une cible accessible | 2 | 1 |

**Gain observé lors de la dernière reproduction : 2,39×, soit environ 58 % de temps en moins.** Les séries précédentes ont donné 2,36–2,50×. Ce chiffre concerne cette fixture locale et un cache chaud. Il ne représente ni un NAS, ni un disque froid, ni une mesure de bout en bout de la fenêtre native. La création de vrais symlinks de fichiers a été tentée, mais refusée par Windows (1314) ; les jonctions constituent une mesure réelle du même parcours de scan, pas un substitut à la validation de tous les types de symlinks.

La réduction des lignes DOM est vérifiée dans le navigateur ; aucun pourcentage de gain de temps frontend n'est revendiqué. Le scan reste proportionnel au nombre total d'entrées de l'arborescence, y compris les fichiers ordinaires. La latence de `metadata` sur des volumes indisponibles peut rester dominante ; huit workers bornent la concurrence mais ne garantissent pas de délai maximal par appel système.

## Vérifications

- `svelte-check` : zéro erreur, zéro avertissement.
- Build Vite de production et compilation Windows release.
- `cargo clippy --lib -- -D warnings`.
- Tests Rust : validation des imports/remappage, racine `/`, Unicode et UNC, conservation de l'original après échec de création.
- Benchmark : équivalence complète des 3 000 résultats avant/après et 300 cassés.
- UI avec IPC simulé : 100 lignes, passage page 2, recherche avec exclusions, export de 209 résultats (toutes les pages), aperçu auto, racine cliquable, remappage, annulation, confirmation et résultat 2 999 réussites/1 échec.
- Inspection visuelle des écrans vides/remplis et de l'import ; fenêtre de 700 px sans débordement horizontal global.

Les tests UI utilisent explicitement `--mode test` et ne touchent pas le filesystem. La fixture n'est pas embarquée dans la build de production. Les tests ne constituent pas une validation native complète du dialogue UAC, du WebView Linux ou de cibles réseau.

## Reproduction

```powershell
npm run check
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --lib -- -D warnings
# Windows / PowerShell 7 : jonctions, pas de privilège symlink nécessaire
./tests/benchmark.ps1
# UI avec 3 000 entrées simulées
node node_modules/vite/bin/vite.js --mode test --port 1422
```

Pour un benchmark de véritables symlinks, sur Linux ou Windows avec le privilège requis : `cargo test --manifest-path src-tauri/Cargo.toml --lib benchmark_scan_3000 -- --ignored --nocapture`. Ne pas définir `SYMLINK_BENCH_ROOT` dans ce cas.

L'environnement présent possède un raccourci `npm` cassé dans le profil utilisateur. Les vérifications ont employé les CLI locales via `node` ou le npm de `C:/Program Files/nodejs`. Le projet ne nécessite pas de changement pour cette anomalie locale.

## Suite recommandée, par priorité

1. **Validation plateforme** : tests natifs Windows/UAC (acceptation, refus, volumes réseau) et Linux ; CI sur les deux systèmes. Une build Windows ne garantit pas la parité runtime Linux.
2. **Scans réseau longs** : progression et annulation coopérative, mesures séparées parcours/validation/IPC sur le vrai dataset de l'utilisateur. Étudier des délais et la stratégie réseau seulement après ces mesures.
3. **Élévation et concurrence externe** : durcir le transport du job élevé contre une modification par un autre processus du même utilisateur. Les vérifications de chemins ne sont pas une protection absolue contre une course filesystem entre contrôle et écriture. La sauvegarde protège un échec normal, pas une coupure entre deux opérations ; un journal de récupération serait utile pour cet autre niveau de garantie.
4. **Opérations très longues** : suivi persistant des jobs UAC au-delà du polling historique de 60 secondes ; reprise/consultation plutôt qu'un simple message de délai dépassé.
5. **Très gros imports** : limite de taille JSON, prévalidation des collisions parent/enfant et export par fichier temporaire + remplacement atomique. Inutile d'ajouter une base de données ou un framework d'état pour les 2 000–3 000 liens visés.
6. **Maintenance** : audit de vulnérabilités des dépendances et mises à jour contrôlées dans un chantier distinct. Les dépendances ont été inspectées pour leur usage ; aucune certification d'absence de CVE n'est revendiquée.

## Commit proposé

`feat: redesign symlink workspace and optimize safe bulk operations`
