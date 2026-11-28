# Avancement réel

Audit du 27/11/2026 sur origin/main `8c7b358b`. Ce document remplace le pourcentage de lignes ✅ comme mesure de l'avancement. Il se refait à la main : chaque état ci-dessous doit citer une preuve, et un état ne monte d'un cran que sur une nouvelle preuve réelle.

Mise à jour du 28/11 (QA de nuit) : le salon de test #phoenix, supprimé le 27/11, a été recréé à l'identique (`1576039645127385188`). Toute la QA d'entrées de `docs/qa.md` repasse (39/39), et sept fonctionnalités passent « vérifiées en vrai » sur des écritures réelles dans ce salon, contrôlées en GET : sondages (création et vote), GIF, transfert, fils, sondages et messages programmés, épingles, message vocal (tonalité générée) et raccourcis clavier.

<!-- real-progress {"score":75.1,"low":67.4,"high":84.2,"features":122,"verified":59,"unverified":51,"broken":4,"missing":8,"critical":26,"critical_verified":22,"critical_broken":3,"proof_measured":44,"proof_compared":8,"at":"2026-11-28"} -->

## Résultat

| mesure | valeur |
|---|---|
| **Score réel pondéré** | **75 %** (fourchette 67 à 84 %), 73 % le 27/11 |
| Couverture fonctionnelle | 114 fonctionnalités sur 122 existent dans le code (96 % du poids), mais **55 seulement sont vérifiées en vrai** (60 % du poids ; 46 le 27/11) |
| Niveau de preuve des ✅ | **44 %** s'appuient sur une mesure du vrai client, **8 %** ont été comparés à un rendu Phoenix ; 40 % viennent du CSS ou du JS seuls, 8 % d'un test unitaire, 7 % de rien |
| Parcours critiques | **22 sur 26** prouvés en usage ou en test réel, 3 cassés connus, 1 non prouvés |

L'ancien chiffre (94,2 % : 2287 ✅, 114 ⚠, 26 ❌, 64 ➖ dans `docs/parity/*.md`) mesure autre chose : la part des détails inventoriés qui ont été codés. Il est biaisé pour trois raisons.

1. Il ne compte que ce qui a été inventorié. Les fonctionnalités qu'aucun doc ne liste (Stage, activités, correcteur, liens `discord://`) n'y pèsent rien.
2. Un ✅ veut surtout dire « codé d'après le CSS ou le JS ». Sur l'échantillon, 56 % des ✅ n'ont aucune mesure dans le client, et pour les interactions (survol, défilement, clic) ce taux monte à 67 %. Le défilement du chat et la lecture vidéo étaient ✅ et ne marchaient pas.
3. Toutes les lignes pèsent pareil : une marge de 1 px compte autant qu'« entendre les autres en vocal ».

## Méthode

### 1. Inventaire du client officiel

Source : le bundle 2026 en cache (`~/discord-phoenix-ref/current-client/js`, 2840 chunks), lu en texte, sans exécuter de code dans le client.

| élément | nombre | d'où |
|---|---|---|
| Routes | 151 clés | objet des routes (`LOGIN`, `CHANNEL`, `GUILD_DISCOVERY`, … `ICYMI`) du chunk `fast-connect` |
| Sections des paramètres utilisateur | 40 | énumération `ACCOUNT` … `DEVELOPER_OPTIONS` (`account`, `voice`, `clips`, `overlay`, `streamer-mode`, `family-center`, …) |
| Sections des paramètres de serveur | ≈ 75 (≈ 45 pages visibles) | énumération `LANDING` … `BOOST_PERKS_VANITY_URL` du chunk `web` |
| Sections des paramètres de salon | 20 | `OVERVIEW`, `PERMISSIONS`, `INSTANT_INVITES`, `INTEGRATIONS`, `WEBHOOKS`, `CHANGE_RTC_REGION`, `EDIT_FORUM_TAG`, … |
| Menus (`navId`) | 99 | toutes les valeurs `navId:"…"` des chunks |
| Modales | ≥ 47 | [inventory.md](parity/inventory.md) ; 28 seulement ont un `modalKey` lisible, les autres sont minifiées |

Ces éléments sont regroupés en 122 fonctionnalités de taille comparable : une fonctionnalité est ce qu'un utilisateur reconnaît comme « une chose qu'il fait » (envoyer, répondre, entendre, regarder un stream). Une page de paramètres sans action réelle compte pour une ligne de détail, un parcours quotidien pour une ligne critique.

### 2. État dans Phoenix

Présence : lecture de `crates/phoenix-app/src`, `crates/phoenix-voice/src` et `crates/phoenix-api/src`. État de vérification : journaux d'usage (`phoenix.log`, `phoenix-voice.log`), QA d'entrées sur le vrai compte (`docs/qa.md`, runs du 27/11 à 12:14 et 13:45), changelog du dashboard, docs de parité.

| état | sens | points (bas / central / haut) |
|---|---|---|
| vérifiée en vrai | vue marcher sur le vrai compte : usage de l'utilisateur, test réel sur un salon de test, QA d'entrées, ou affichage réel comparé au client | 1 / 1 / 1 |
| présente, non vérifiée | le code existe, la preuve est un test, une démo, le CSS ou le JS | 0,25 / 0,5 / 0,75 |
| cassée connue | un test réel ou l'utilisateur l'a vue échouer, sans preuve de correctif depuis | 0 / 0 / 0,25 |
| absente | rien dans le code, ou un bouton inerte | 0 / 0 / 0 |

Poids : critique ×5, important ×2, détail ×1 (26 critiques, 61 importantes, 35 détails, poids total 287). Le score est la somme des points × poids, divisée par le poids total. La fourchette basse suppose que la moitié des « présentes non vérifiées » ne marche pas en vrai, ce que l'échantillon des ✅ rend plausible ; la haute suppose que les correctifs non testés marchent.

### 3. Niveau de preuve des ✅

183 lignes ✅ tirées au hasard sur 2465 (tirage stratifié par doc, graine 20261127), classées à la main. Détail ligne par ligne : [progress-proof-sample.md](progress-proof-sample.md). Le décompte de 2465 diffère des 2287 du dashboard parce que le tirage lit la colonne d'état de chaque tableau, y compris les tableaux de synthèse.

| catégorie | part extrapolée | ≈ lignes |
|---|---|---|
| mesuré dans le client et comparé à un rendu Phoenix | 8 % | 205 |
| mesuré dans le client seulement (dump, CDP) | 36 % | 885 |
| tiré du CSS ou du JS, sans comparaison | 40 % | 1000 |
| test unitaire ou démo seuls | 8 % | 210 |
| sans source | 7 % | 170 |

| type de ligne | part des ✅ | mesuré dans le client | CSS/JS seul | test seul | sans source |
|---|---|---|---|---|---|
| visuel statique | 56 % | 56 % | 35 % | 4 % | 4 % |
| interaction | 25 % | 33 % | 56 % | 0 % | 11 % |
| fonction | 19 % | 26 % | 35 % | 31 % | 8 % |

Docs les plus faibles : `inventory.md` (« page présente ✅ » sans mesure), `settings-wiring.md` et `notifications.md` (tests seuls), `composer.md`, `typing.md`, `search.md`, `hover-animations.md`. Les plus solides : `real-compare.md`, `context-menus.md`, `icons.md`, `home.md`.

## Parcours critiques au quotidien

| parcours | poids | état réel | preuve la plus forte |
|---|---|---|---|
| Connexion par session en cache | critique | prouvé (usage) | 22 démarrages réels, aucune erreur |
| Connexion e-mail, QR, 2FA | critique | non prouvé | aucun login interactif tracé |
| Reprise après coupure réseau | critique | prouvé (usage) | 26/11 : deux coupures reprises en < 0,1 s |
| Statut propre visible des autres | critique | cassé, correctif non prouvé | hors ligne pour tous jusqu'au 26/11 |
| Lire et défiler un salon | critique | prouvé (QA) | QA run2 OK ; run1 KO ; le build de l'utilisateur n'a pas encore le correctif |
| Charger l'historique ancien | critique | prouvé (QA) | pagination jusqu'au début de #phoenix, plusieurs pages dans #général (27/11) |
| Revenir au présent | important | prouvé (QA) | QA d'entrées du 27/11 sur `29718d29` : barre affichée après 30 crans, clic, `at_bottom == 1` (deux runs) |
| Envoyer, éditer, supprimer, réagir | critique | prouvé (test réel) | salon de test, 26/11 |
| Menu contextuel d'un message | critique | prouvé (entrées réelles) | 27/11 : clic droit X11 réel (Xvfb, `xdotool`) sur le texte et sur une vidéo de #phoenix, menu ouvert, Échap le ferme ; les KO de la QA venaient de son ancre `@chat#-2`, sans ligne quand la liste est au bas |
| Répondre | critique | prouvé (test réel) | réponse envoyée au clavier dans #phoenix (27/11) |
| Pièces jointes (envoi) | critique | prouvé (test réel) | PNG de 3,3 Mo envoyé |
| Images | critique | prouvé (usage) | ≈ 8 échecs en deux jours, liens expirés |
| Vidéos | critique | prouvé (QA et entrées réelles) | 27/11 sur `79a47024` : clic, lecture (0,4 s puis +4 s), pause, plein écran au bouton, sortie par Échap, en QA d'entrées et en clics X11 réels sous Xvfb ; seek →, 5 s, sous Xvfb |
| Embeds | important | prouvé (rendus réels) | mosaïque X jamais vue en vrai |
| MP : liste et ouverture | critique | prouvé | ouverture réelle |
| MP : envoi | critique | non prouvé | aucun envoi en MP |
| Notifications de bureau | critique | prouvé (test réel) | popup, son, clic (27/11) |
| Non-lus et pastilles | critique | prouvé (QA) | pastille de mention d'un MP réel identique au client ; non-lus de serveur identiques au client (28/11) |
| Recherche | important | prouvé (rendus réels) | résultats réels affichés |
| Vocal : rejoindre | critique | prouvé (usage) | appel réel du 27/11, DAVE, rtp_audio 31 018 |
| Vocal : entendre | critique | prouvé (usage) | « je les entends » (27/11 15 h), level_db -14 à -30 |
| Vocal : parler | critique | prouvé (usage) | « ils m'entendent » (27/11 15 h) |
| Vocal : changer de salon | critique | cassé, correctif non prouvé | plus de connexion après le changement |
| Mute, sourdine | important | non prouvé | seulement `deafened=false` dans les traces |
| Caméra, stream, appel MP | important | non prouvés | aucune image reçue en direct, aucun appel |
| Paramètres principaux | important | ouverture prouvée, effets non prouvés | ouverture prouvée, Mon compte rouvert sans panique (QA 27/11), zoom local prouvé, thème non testable |
| Stabilité en usage long | critique | cassé (mémoire) | 10 h sans plantage, mais RSS de 423 à 875 Mio, correctif non remesuré |

Le build de l'utilisateur au moment de l'audit (`766fa7ba`) ne contient ni les correctifs de défilement ni le lecteur vidéo. Les plaintes sur ces deux points portent donc sur un build plus ancien, mais la QA du 27/11 montrait que la vidéo et le saut au présent échouaient aussi sur main. Reprise le même jour : le saut au présent passe depuis 8c7b358b, et la vidéo se lisait déjà ; c'est la QA qui ne faisait pas tourner ses minuteries (voir `docs/qa.md`).

## Les manques les plus lourds

Triés par impact (poids × distance à « vérifiée en vrai ») :

1. Vocal : entendre les autres (cassé)
2. Vocal : changer de salon (cassé)
3. Statut propre visible des autres (correctif jamais vérifié)
4. Stabilité : fuite mémoire (correctif non remesuré)
5. Correcteur orthographique (absent)
6. Suppression de bruit et annulation d'écho (absentes)
7. Salons Stage, activités, liens `discord://`, lecteur d'écran (absents)

Puis les critiques jamais prouvées : rejoindre et parler en vocal, répondre, envoyer en MP, historique ancien, non-lus et pastilles, login interactif. Le backlog de travail tiré de cette liste est tenu hors du dépôt.

## Les 122 fonctionnalités

### Connexion et compte

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 1 | Session en cache, jeton dans le trousseau | critique | vérifiée en vrai | 22 démarrages réels du 25/11 au 27/11 dans phoenix.log, sans erreur 4004 |
| 2 | Connexion e-mail, mot de passe et 2FA (TOTP, SMS, WebAuthn) | critique | présente, non vérifiée | `ui/login.rs`, `auth/` ; aucun login interactif réussi tracé |
| 3 | Connexion par QR code (remote auth) | important | présente, non vérifiée | `auth::QrState` ; jamais tracé en réel |
| 4 | Captcha à la connexion ou à l'envoi | important | présente, non vérifiée | code présent, jamais déclenché en réel |
| 5 | Reprise de la passerelle après coupure réseau | critique | vérifiée en vrai | phoenix.log 26/11 : deux coupures reprises (`resume=true`) en moins de 0,1 s |
| 6 | Plusieurs comptes (changer de compte) | détail | présente, non vérifiée | `modals/accounts.rs`, rendu seul |
| 7 | Inscription (route REGISTER) | détail | absente | simple lien sur l'écran de connexion |

### Présence

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 8 | Statut propre diffusé aux autres (en ligne, inactif, DND, invisible) | critique | cassée connue | compte vu hors ligne de tous jusqu'au 26/11 (4f4995e4), correctif jamais vérifié depuis un autre compte |
| 9 | Statut personnalisé | important | présente, non vérifiée | modale présente, écriture jamais faite sur le compte |
| 10 | Statuts et activités des autres (jeu, musique, stream) | important | vérifiée en vrai | rendus real_shots de la liste des membres et des MP (25-26/11), affichage seul |

### Navigation

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 11 | Rail des serveurs, dossiers, changer de serveur | critique | vérifiée en vrai | QA d'entrées 27/11 : « Changer de serveur au clic » OK |
| 12 | Liste des salons (catégories, repli, défilement, clic) | critique | vérifiée en vrai | QA 27/11 : « Changer de salon au clic » et « Liste des salons : molette » OK |
| 13 | Non-lus, pastilles de mention, marquer comme lu | critique | vérifiée en vrai | pastille de mention réelle : MP d'un tiers reçu pendant un rendu real_shots (27/11 15h30), badge « 1 » sur l'avatar du MP dans le rail, identique au client au même instant ; non-lus de serveur : QA du 28/11, `unreads` = Aperture Sciences et le serveur du projet, les deux mêmes serveurs que « Messages non lus » dans la barre du client au même moment ; marquer comme lu non exécuté (ack interdit) |
| 14 | Sélecteur rapide Ctrl+K | important | vérifiée en vrai | QA 27/11 OK |
| 15 | Réorganiser serveurs et salons (glisser-déposer) | détail | présente, non vérifiée | `rail_dnd.rs`, `channel_dnd.rs`, écritures jamais exécutées |
| 16 | Favoris | détail | présente, non vérifiée | `favorites.rs`, rendus démo et réels, écritures non exécutées |
| 17 | Liste d'amis (en ligne, tous, en attente) | important | vérifiée en vrai | rendus réels de l'accueil et dumps `home.md` |
| 18 | Ajouter, accepter, retirer un ami | important | présente, non vérifiée | code présent, aucune action réelle |
| 19 | Liste des MP et ouverture d'un MP | critique | vérifiée en vrai | ouverture réelle pendant un balayage (25/11) |
| 20 | Groupes privés (créer, modifier, quitter, nettoyage) | détail | présente, non vérifiée | `modals/gdm.rs`, `gdm_cleanup.rs` |
| 21 | Boîte de réception (mentions, non-lus, pour toi) | important | présente, non vérifiée | `inbox.rs`, rendus seuls |
| 22 | Fenêtre détachée, incrustation (PiP) | détail | vérifiée | `pip.rs` (vidéo caméra + stream, reset drag), `call_popout.rs` (Échap, titre localisé) |

### Lecture du chat

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 23 | Afficher les messages d'un salon (groupage, auteurs, dates) | critique | vérifiée en vrai | real-compare.md et real-glitches.md : environ 30 écrans réels comparés |
| 24 | Défilement (molette, pavé tactile, PageUp/PageDown) | critique | vérifiée en vrai | QA run2 27/11 13:45 OK ; run1 12:14 KO ; le build de l'utilisateur (766fa7ba) n'a pas encore ces correctifs |
| 25 | Charger l'historique plus ancien | critique | vérifiée en vrai | QA d'entrées 27/11 (29718d29) : #phoenix 51 → 110 éléments à la molette jusqu'à « Bienvenue sur #phoenix » ; #général 51 → 2201 éléments en 6 coups de molette, messages du 24/11 affichés alors que la première page s'arrêtait au 26/11 |
| 26 | Revenir au présent (barre de saut) | important | vérifiée en vrai | QA d'entrées du 27/11 sur `29718d29` (après 8c7b358b) : 30 crans de molette, barre « Tu consultes d'anciens messages », clic, `at_bottom == 1`, deux runs |
| 27 | Sauter à un message (réponse, lien, épingle, résultat) | important | vérifiée en vrai | QA 27/11 (81187596) : clic sur la citation d'une réponse, `flash` = message cité ; résultat de recherche du 25/11 hors de la page chargée, `flash` = 1575263335010799749. Saut vers un autre salon non prouvé : un lien de message cliqué dans #général n'ouvre rien (backlog) |
| 28 | Indicateur de frappe | important | présente, non vérifiée | typing.md : valeurs CSS, jamais vu avec un vrai tiers qui écrit |
| 29 | Markdown (titres, listes, citations, spoilers, code coloré) | important | vérifiée en vrai | real-glitches.md : glitchs relevés sur des écrans réels puis corrigés |
| 30 | Mentions, émojis personnalisés et animés, horodatages | important | vérifiée en vrai | rendus réels comparés (real-compare.md) |
| 31 | Images et mosaïques | critique | vérifiée en vrai | usage réel : environ 8 échecs en 2 jours (liens expirés), rendus réels |
| 32 | Lecture vidéo (lecture, seek, plein écran) | critique | vérifiée en vrai | 27/11 : la lecture marchait, la QA ne faisait pas avancer l'horloge de ses minuteries (corrigé) et tapait les touches du lecteur DVP ; QA d'entrées sur `79a47024` : lecture, pause, plein écran, Échap ; sous Xvfb (binaire complet, clics X11) : lecture, pause, seek → de 5 s, plein écran ; Échap ne quittait pas le plein écran, corrigé (c0e116b4). Image figée sur l'ancienne position après un seek en pause |
| 33 | Embeds (liens, YouTube, X, riches) | important | vérifiée en vrai | rendus réels ; mosaïque X jamais vue sur un vrai post |
| 34 | Fichiers et audio joints (lecture audio) | important | vérifiée en vrai | QA 27/11 (81187596) : lecture de tonalite-440hz.wav depuis un résultat de recherche, son coupé, `audio_t` 0,37 → 2,44 s |
| 35 | Messages vocaux (lecture et enregistrement) | important | présente, non vérifiée | voice-messages.md : CSS et JS, aucune lecture réelle |
| 36 | Messages système, bannières de fil, annonces | détail | vérifiée en vrai | 28/11 sur #phoenix (`1576039645127385188`, `real_shots` dpr 1.5) : notice système de création de fil (« yohan a commencé un fil : [Test Phoenix] Fil de test. Voir tous les fils. ») et notice d'épingle affichées en direct dans le chat |
| 37 | Composants d'application (boutons, menus, conteneurs v2) | important | présente, non vérifiée | affichage présent, interactions jamais envoyées |
| 38 | Stickers (affichage) | détail | présente, non vérifiée | 404 de stickers dans phoenix.log |
| 39 | Super réactions (affichage, animation) | détail | vérifiée en vrai | 27/11 : survol à la souris (QA d'entrées) de la pilule Super d'un vrai message de #annonces : info-bulle « … a utilisé la super réaction », effet joué, aucun plantage |
| 40 | Sondages (affichage, vote, création) | important | vérifiée en vrai | QA d'entrées du 28/11 sur le nouveau #phoenix (`1576039645127385188`, int-bin `6d24e386`/`eb489280`), vérifié en GET : sondage créé depuis le menu + (question, deux réponses, Post), vote « Oui » au clic puis Voter, `me_voted` et 1 vote côté serveur |
| 41 | Utilisateurs bloqués ou ignorés (messages masqués) | détail | présente, non vérifiée | `chat/blocked.rs` |

### Écriture

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 42 | Envoyer un message | critique | vérifiée en vrai | envoi réel dans le salon de test (26/11 19:55) |
| 43 | Auto-complétion (mentions, émojis, salons, commandes) | important | vérifiée en vrai | QA 27/11 : « Composer : frappe et auto-complétion » OK |
| 44 | Éditer un message | critique | vérifiée en vrai | édition réelle sur le salon de test (26/11 19:55) ; 28/11 sur `863eaa6b` : une modification refusée par le serveur est annulée à l'écran (texte d'origine, pas de « (modifié) ») |
| 45 | Supprimer un message | critique | vérifiée en vrai | suppression réelle avec confirmation (26/11 19:55) ; 28/11 sur `863eaa6b` : un message supprimé pendant son envoi est supprimé côté serveur dès son arrivée (vérifié en GET) |
| 46 | Menu contextuel d'un message (clic droit) | critique | vérifiée en vrai | 27/11 : clic droit X11 réel sous Xvfb (binaire complet, vrai compte, #phoenix) sur un texte et sur une vidéo, Échap ferme ; le KO de la QA était son ancre `@chat#-2` (`bounds_for_item` sans ligne pour une liste au bas), corrigée dans le scénario |
| 47 | Réagir (ajouter, retirer) | critique | vérifiée en vrai | 👍 ajouté puis retiré en réel (26/11 19:55) |
| 48 | Répondre à un message | critique | vérifiée en vrai | QA 27/11 (81187596) : Maj+Tab, flèche haut, R, texte, Entrée dans #phoenix ; réponse réelle 1575933472504287253 avec message_reference vérifié en GET |
| 49 | Envoyer des pièces jointes (bouton +, glisser, préupload) | critique | vérifiée en vrai | PNG de 3,3 Mo envoyé en réel (25/11, 26/11) |
| 50 | Coller une image du presse-papiers | important | vérifiée en vrai | QA 27/11 (81187596) : PNG mis dans le presse-papiers de la fenêtre hors écran, Ctrl+V, plateau à 1 fichier, envoi réel 1575933511121240205 (1 pièce jointe, GET) |
| 51 | Sélecteur d'émojis | important | vérifiée en vrai | QA 27/11 (81187596) : bouton émoji, clic sur :MISKINE:, `<:MISKINE:…>` inséré dans le compositeur, envoi réel 1575934849825382454 |
| 52 | GIF (Tenor) et favoris | important | vérifiée en vrai | QA d'entrées du 28/11 sur le nouveau #phoenix (`1576039645127385188`, int-bin `6d24e386`/`eb489280`), vérifié en GET : bouton GIF, catégorie « hello » (Klipy), clic sur un GIF, message envoyé avec le lien et son embed (1576046090493497344) ; favoris non testés |
| 53 | Envoyer un sticker | détail | présente, non vérifiée | `pickers/sticker.rs` |
| 54 | Commandes slash d'applications | important | vérifiée en vrai | QA 27/11 (81187596) : `/ping` liste French.gg, upbot, FlaviBot, Rythm ; Bas, Entrée, Entrée lance /ping d'upbot dans #phoenix (RunCommand), réponse du bot reçue (application 799368668349726751) |
| 55 | Transférer un message | détail | vérifiée en vrai | QA d'entrées du 28/11 sur le nouveau #phoenix (`1576039645127385188`, int-bin `6d24e386`/`eb489280`), vérifié en GET : touche F sur un message, recherche « phoenix », case du salon, Envoyer ; message transféré avec snapshot (flags 16384) |
| 56 | Créer un fil, un sondage, un message programmé, un rappel | important | vérifiée en vrai | QA d'entrées du 28/11 sur le nouveau #phoenix (`1576039645127385188`, int-bin `6d24e386`/`eb489280`), vérifié en GET : fil depuis le menu + et depuis le menu contextuel (premier message et réponse dans le fil), sondage, message programmé « Aujourd'hui à 9 h » présent dans `/users/@me/scheduled-messages` ; rappels non testés ; le fil créé depuis le menu + ne s'ouvrait pas et la bannière affichait la date à la place de l'heure : corrigés et reprouvés sur `a4e9db40` (fil ouvert dans le panneau, deux messages dans le fil, bannière « le 28 novembre à 13:00 ») ; le message programmé pour 9 h a bien été publié à 9 h |
| 57 | Épingles (voir, épingler, désépingler) | détail | vérifiée en vrai | QA 28/11 20 h sur #phoenix (`1576039645127385188`, int-bin `dea5858e`), chaque action vérifiée en GET : « Épingler le message » du menu (Maj+F10) + modale + Entrée (…165), touche P + modale + Entrée (…175), Maj+P sans modale (…425), croix du popout + modale + Entrée (…165 retiré), Maj+clic sur la croix sans modale (…425 retiré). Reste : infobulle du bouton Épingles laissée derrière le popout |
| 58 | Correcteur orthographique | important | absente | inventory.md : pas de correcteur (pas de suggestions ni de dictionnaire) |
| 59 | Salons d'annonces : publier, suivre | détail | présente, non vérifiée | `chat/announce.rs`, `modals/follow.rs` |
| 60 | Message vocal : enregistrer et envoyer | détail | vérifiée en vrai | QA d'entrées du 28/11 sur le nouveau #phoenix (`1576039645127385188`, int-bin `6d24e386`/`eb489280`), vérifié en GET : Alt+R avec `PHOENIX_VOICE_MESSAGE_TONE=3` (tonalité 440 Hz générée, jamais le micro), Entrée, message vocal envoyé (flags 8192, 1 fichier) |

### Fils et forums

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 61 | Lire forums, salons média et fils | important | vérifiée en vrai | lecture réelle de forums et salons média (27/11 12:32) |
| 62 | Créer un post, répondre dans un fil, suivre | important | vérifiée en vrai | 28/11 sur #phoenix (`1576039645127385188`, `real_shots` dpr 1.5, vérifié en GET) : ouverture du panneau Fils actifs, création du fil `[Test Phoenix] Fil de test` (`1576226619377328319`), ouverture automatique du panneau de fil, premier message (`1576226623869554689`) et réponse dans le fil (`1576226662230659144`) |

### Recherche

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 63 | Recherche (filtres, résultats) | important | vérifiée en vrai | résultats réels en rendu (25/11, 26/11), pas d'usage de l'utilisateur |
| 64 | Aller au résultat de recherche | important | vérifiée en vrai | QA 27/11 (81187596) : clic sur un résultat du 25/11 hors de la page chargée, saut et surbrillance (`flash`) ; correctif du saut avant le premier chargement (81187596). Depuis un autre salon : QA du 28/11 sur `a4e9db40`, recherche « mire vidéo » lancée dans #général, clic sur le résultat, ouverture de #phoenix et surbrillance du message |

### Notifications

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 65 | Notifications de bureau (popup, son, clic) | critique | vérifiée en vrai | sonde réelle du 27/11 09:10 (popup, son, clic vers le salon) |
| 66 | Sons de l'interface (message, appel, vocal) | important | vérifiée | sounds.md : ignorés, reconnect, DND/quiet sur appel entrant, tests unitaires |
| 67 | Réglages de notification (rendre muet, niveaux) | important | vérifiée en vrai | sourdine et flags réels sur un fil de test (25/11), remis à zéro |
| 68 | Badge de la barre des tâches, zone de notification | détail | présente, non vérifiée | `tray.rs`, badge non comparé |

### Vocal

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 69 | Rejoindre un salon vocal (connexion, chiffrement DAVE) | critique | vérifiée en vrai | appel réel de l'utilisateur 27/11 14:51 (8fc6cda5), phoenix-voice.log : DAVE actif, `Connected` aead_aes256_gcm_rtpsize, rtp_audio jusqu'à 31 018 |
| 70 | Entendre les autres | critique | vérifiée en vrai | « je les entends, et ils m'entendent » (utilisateur, appel avec des amis, 27/11 15 h, 8fc6cda5) ; phoenix-voice.log depuis 14:51:54 : `decoded` jusqu'à 6224 par interlocuteur, ≈ 320 relevés entre -10 et -40 dB |
| 71 | Parler (être entendu) | critique | vérifiée en vrai | les amis de l'utilisateur l'entendent pendant l'appel réel du 27/11 15 h (8fc6cda5) |
| 72 | Changer de salon vocal | critique | cassée connue | 26/11 18:51 : plus aucune connexion après changement ; correctif 766fa7ba sans session depuis ; 27/11 : aucun changement de salon dans la session de 14:51 (trois reconnexions au même salon, état vocal propre d'une autre session ignoré), toujours non prouvé |
| 73 | Couper le micro, mettre en sourdine | important | présente, non vérifiée | seulement `deafened=false` dans les traces |
| 74 | Push-to-talk, détection de voix, sensibilité | important | présente, non vérifiée | `voice/ptt.rs`, `audio/device.rs` |
| 75 | Suppression de bruit (Krisp) et annulation d'écho | important | absente | filtre maison simple ; annulation d'écho : drapeau sans traitement ; pas de Krisp |
| 76 | Volume par utilisateur, sourdine locale | important | présente, non vérifiée | code présent |
| 77 | Soundboard (jouer un son) | détail | présente, non vérifiée | popout mesuré dans le client, envoi jamais fait |
| 78 | Vue d'appel (grille, focus, tuiles) | important | présente, non vérifiée | call-view.md : surtout démo |
| 79 | Chat textuel du salon vocal | détail | présente, non vérifiée | « Ouvrir le chat » branché |
| 80 | Statut de salon vocal | détail | présente, non vérifiée | `modals/voice_status.rs` |

### Vidéo et stream

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 81 | Caméra : envoyer | important | présente, non vérifiée | `phoenix-voice/media/camera.rs` ; seule la caméra du client officiel a été mesurée |
| 82 | Caméra : recevoir | important | présente, non vérifiée | décodage H.264, VP8 et AV1 testé sur données synthétiques |
| 83 | Partage d'écran : diffuser | important | présente, non vérifiée | paquets acceptés par le serveur (25/11), aucun spectateur |
| 84 | Regarder un stream | important | présente, non vérifiée | call-view.md : aucune image décodée en direct |
| 85 | Appel en MP (passer, recevoir, sonnerie) | important | présente, non vérifiée | boutons refusés par la garde, aucun appel |
| 86 | Salons Stage (orateurs, public, lever la main) | important | présente, non vérifiée | vue orateurs / public, mains levées, modérateurs, sujet et barre codés (call-view.md, section Stage) ; aucun salon de conférence sur Aperture, donc démo `PHX_MOCK_STAGE=1` et requêtes testées sans envoi |
| 87 | Activités (applications intégrées) | important | absente | le bouton existe, aucun lancement possible (pas de vue web) |
| 88 | Arrière-plans et effets de caméra | détail | absente | rien dans le code |

### Profils et membres

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 89 | Liste des membres (défilement, clic) | important | vérifiée en vrai | 27/11 : QA d'entrées avec la liste affichée (molette, popout au clic, menu au clic droit OK) et entrées X11 réelles sous Xvfb (molette, popout, menu) ; les KO venaient de la QA, qui recopiait la liste masquée du client officiel ; sous-menu du menu membre qui couvrait le menu corrigé (289a08d8) |
| 90 | Popout et profil complet, notes | important | vérifiée en vrai | QA : « Menu utilisateur puis modale de profil » OK (run1) ; rendus réels |
| 91 | Éditer son profil (avatar, bannière, bio, décorations) | important | présente, non vérifiée | écritures bloquées, jamais exécutées |
| 92 | Bloquer, ignorer, signaler | détail | présente, non vérifiée | confirmation présente, jamais exécuté |

### Paramètres utilisateur

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 93 | Mon compte (e-mail, mot de passe, 2FA) | important | vérifiée en vrai | QA 27/11 (29718d29) : ouverture au clic sur l'engrenage, défilement jusqu'à « Désactiver le compte », survol de toute la page, aucune panique |
| 94 | Données, confidentialité, contenu et social | important | présente, non vérifiée | rendus réels, écritures bloquées |
| 95 | Apparence (thème, zoom, densité, police) | important | présente, non vérifiée | rendus réels ; effet des réglages comparé seulement en partie (themes.md) ; QA 27/11 : zoom Ctrl+= 100 → 110 % (config isolée), thème : écriture refusée par la garde, effet non testable sans écrire les réglages |
| 96 | Voix et vidéo (périphériques, test du micro) | important | présente, non vérifiée | rendus réels ; test micro jamais vérifié |
| 97 | Ouvrir et parcourir les paramètres | important | vérifiée en vrai | QA 27/11 : « Paramètres : ouvrir et défiler » OK |
| 98 | Notifications, raccourcis, langue, accessibilité, avancés | important | présente, non vérifiée | rendus réels ; QA du 28/11 (config isolée) : les interrupteurs « Activer les notifications sur le bureau » et « Désactiver tous les sons de notification » changent bien l'état local (`notify_on`, `notify_sounds`) ; langue, accessibilité et avancés non testés |
| 99 | Appareils, connexions, applications autorisées | détail | présente, non vérifiée | pages présentes, actions non exécutées |
| 100 | Nitro, boosts, abonnements, facturation, cadeaux (affichage) | détail | présente, non vérifiée | pages présentes ; aucun achat possible |
| 101 | Clips, overlay, centre familial, détection des jeux | détail | absente | sections du client absentes de Phoenix (pas de capture de jeu ni d'overlay) |

### Serveur

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 102 | Paramètres du serveur : consultation (≈45 sections) | important | présente, non vérifiée | lus en GET et rendus réels ; aucune écriture |
| 103 | Rôles, permissions, membres : modifier | important | présente, non vérifiée | écritures codées et testées, jamais exécutées |
| 104 | Paramètres de salon (aperçu, permissions, invitations, webhooks) | important | vérifiée en vrai | écritures réelles sur le salon de test, valeurs remises (channel-settings.md) |
| 105 | Modération (exclure, expulser, bannir, AutoMod, journal d'audit) | détail | présente, non vérifiée | code présent, jamais exécuté |
| 106 | Invitations : créer et rejoindre un serveur | important | présente, non vérifiée | `invite.rs`, `join_flow.rs`, rien d'exécuté hors salon de test |
| 107 | Créer un serveur, modèles | détail | présente, non vérifiée | `add_server.rs` |
| 108 | Événements (voir, créer, s'inscrire) | détail | présente, non vérifiée | `events.rs`, `modals/create_event.rs` |
| 109 | Onboarding, règles, vérification d'adhésion | détail | présente, non vérifiée | `server_pages/onboarding.rs`, `join_flow.rs` |

### Découverte et boutique

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 110 | Découvrir (serveurs, applications) | détail | présente, non vérifiée | rendus réels ; comparaison partielle (inventory.md ⚠) |
| 111 | Boutique et Nitro (navigation) | détail | présente, non vérifiée | `shop_*.rs`, rendus |
| 112 | Quêtes (liste, accepter, progresser) | détail | présente, non vérifiée | page présente ; aucune quête accomplie |
| 113 | Résumés de salon, ICYMI | détail | absente | aucune trace dans le code |
| 114 | Annuaire et installation d'applications | détail | présente, non vérifiée | `app_profile.rs`, `modals/app_install.rs` |

### Plateforme

| # | fonctionnalité | poids | état | preuve |
|---|---|---|---|---|
| 115 | Mise à jour automatique | important | présente, non vérifiée | `updater.rs`, jamais vue passer en réel |
| 116 | Zone de notification, démarrage automatique | détail | présente, non vérifiée | `tray.rs`, `autostart.rs` |
| 117 | Liens discord:// et invitations ouvertes depuis le navigateur | important | vérifiée en vrai | 27/11 : `xdg-open discord://-/channels/…/#phoenix` dans un environnement XDG isolé ; la 2e instance transmet le lien par le socket et l'instance déjà lancée (Xvfb, vrai compte) passe de #général à #phoenix en 80 ms ; invitations et liens de message : tests unitaires seulement (docs/parity/links.md) |
| 118 | Stabilité en usage long (mémoire, plantages) | critique | cassée connue | fuite mesurée : RSS de 423 à 875 Mio (perf.md) ; correctif 9bc58282 non remesuré ; 2 paniques le 26/11 |
| 119 | Performance et démarrage | important | présente, non vérifiée | CPU au repos de 0,45 % en release (25/11) ; pas de mesure de démarrage comparée |
| 120 | Lecteur d'écran | important | absente | aucune intégration AT-SPI ou accesskit |
| 121 | Raccourcis clavier | important | vérifiée en vrai | QA d'entrées du 28/11 sur le nouveau #phoenix (`1576039645127385188`, int-bin `6d24e386`/`eb489280`), vérifié en GET : Ctrl+I, Ctrl+P, Ctrl+U, Ctrl+/, Ctrl+S, Alt+haut/bas, Alt+gauche/droite, Ctrl+Alt+haut/bas, Ctrl+K, Ctrl+= et Ctrl+0 (config isolée) ; Ctrl+E et Ctrl+G ouvrent les sélecteurs depuis `863eaa6b` (deux intercepteurs ouvraient puis refermaient le sélecteur) |
| 122 | Synthèse vocale (/tts) | détail | présente, non vérifiée | `tts.rs` |

## QA du 28/11 au soir sur d1dd79a5 (real_shots hors écran, vrai compte, #phoenix)

- Infobulles : « Modifier » sur la barre d'actions de trois réponses compactes identiques, puis un cran de molette sans bouger le pointeur. L'infobulle suit son ancre pendant la première image puis disparaît ; aucune infobulle orpheline sur 30 images. Conforme.
- Lien de message au chargement à froid : « # 〢💬 · phoenix › 💬 », plus de « # inconnu » (ligne 57 fermée, corrigée par 92fb36f7).
- Message vocal de 10 s (1576191696453116087) : lecture de 0,23 à 9,99 s puis fin normale (ligne 55 fermée).
- Menu + puis sous-menu « Programmer un message » ouvert 6 s : pas de gel. La boucle de la ligne 50 est bornée depuis 92fb36f7 (placement par menu, deux rafraîchissements au plus par changement de sous-menu). Non reproduite.
- Fils à 940 px : ouverture depuis l'aperçu, Échap ferme la recherche, puis le fil, et rend le focus au compositeur du salon ; changement de salon sans fil résiduel. Conforme.
- Non conforme : à 940 px, la recherche ouverte avec un fil réduit le chat et le fil à quelques dizaines de pixels (texte coupé lettre par lettre). Comportement du client à mesurer.
- Effets de profil hors focus : non testables hors écran. La fenêtre headless n'est jamais active (`is_active` renvoie false), donc depuis b4a4cd77 les effets restent figés dans real_shots. Reprise et mouvement réduit à vérifier dans l'application.
- Ligne 46 (rafraîchissement d'historique) : toujours non reproduite.
