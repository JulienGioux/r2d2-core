🧹 BRIQUE 11 : ÉLAGAGE DYNAMIQUE ET ENTROPIE D'UTILITÉ 

Gestion de l'Oubli Sélectif et Optimisation de la Densité de Connaissance 

Version :  1.2.0-PRO-SPEC 

Rôle :  Maintenance de la Mémoire à Long Terme (PostgreSQL/pgvector) 

Objectif :  Prévenir la saturation sémantique, éliminer le bruit cognitif et optimiser la latence de recherche vectorielle. 

1. LA PHILOSOPHIE DE L'OUBLI ACTIF : LE MIMÉTISME SYNAPTIQUE 

Dans un système multi-agent comme R2D2, la base de connaissances (Brique 7) croît de manière exponentielle à chaque interaction. Sans un mécanisme d'élagage ( pruning ), le système souffre de &quot;bruit sémantique&quot; : les recherches vectorielles renvoient trop de résultats obsolètes ou contradictoires, ce qui &quot;paralyse&quot; le  Paradox Engine . 

1.1. L'Intelligence par la Soustraction 

L'intelligence n'est pas la capacité de tout stocker, mais celle de filtrer l'essentiel. L'élagage dynamique est un processus biologique simulé (mimétisme du cerveau humain) qui permet de maintenir une  Densité de Connaissance Utile  maximale. En sacrifiant le superflu (fragments spéculatifs, logs de debug, itérations de code ratées), on sauve l'intégrité du noyau de vérité de l'essaim. 

1.2. Le Risque d'Hyperthymésie Synthétique 

Une IA qui se souvient de chaque tentative de code finit par proposer des solutions &quot;fantômes&quot; basées sur d'anciennes versions de bibliothèques. L'oubli actif est donc une mesure de sécurité préventive contre les régressions logiques. 

2. L'ALGORITHME D'UTILITÉ SÉMANTIQUE (ASU) 

Chaque fragment   stocké se voit attribuer une valeur de survie   recalculée dynamiquement par le Kernel durant le cycle circadien (Brique 8). 

2.1. Analyse Approfondie des Variables 

 :  Ce n'est pas un simple compteur. Il représente l'influence du fragment sur le Blackboard. Si un fragment a servi de base à une décision validée par un &quot;Expert Senior&quot;, son score d'usage est multiplié par un coefficient de prestige. 

 :  Score issu de la Brique 1. Un fragment ayant fait l'objet d'un débat intense et validé par une majorité d'agents possède une &quot;inertie de survie&quot; plus forte. Un fragment imposé sans débat est plus fragile. 

 :  Temps écoulé depuis la dernière activation. L'oubli suit une courbe logarithmique : la perte de valeur est rapide au début, puis se stabilise pour les souvenirs qui ont survécu aux premières purges. 

 (Facteur d'Hallucination) :  Malus appliqué si le fragment a été ultérieurement identifié comme erroné par le Paradox Engine. Un fragment avec un   élevé est &quot;marqué pour destruction&quot; immédiate. 

 (Température de l'Oubli) :  *   :  Conservatisme total. Utile pour les phases d'archivage légal. 

 :  Pragmatisme radical. Seule la vérité de l'instant compte. 

3. L'ARCHITECTURE DE LA MÉMOIRE EN STRATES 

L'élagage suit un protocole de dégradation gracieuse en trois étages. 

3.1. Le Cortex Actif (Index HNSW &amp; VRAM) 

Zone de performance extrême (&lt; 10ms). 

Critère :   . 

Action :  Le fragment est maintenu dans l'index vectoriel en RAM. Il est immédiatement disponible pour l'intuition de l'IA. 

3.2. L'Archive Froide (PostgreSQL &amp; NVMe) 

Zone de stockage durable. 

Critère :   . 

Action :  Le fragment est retiré de l'index rapide mais reste dans la base de données. Il est accessible par recherche textuelle (SQL) mais ne pollue plus les résultats de similarité vectorielle. 

3.3. La Distillation Sémantique (Compression de Sagesse) 

Lorsque   est trop faible mais que l'information reste &quot;vraie&quot;, R2D2 procède à une  Distillation . 

Fusion sémantique :  50 fragments détaillant des bugs mineurs sur un projet sont résumés par un agent &quot;Historien&quot; en un seul fragment de type &quot;Leçon Apprise&quot;. 

Pliage (Folding) :  On ne garde que le prédicat (subject-predicate-object) et on détruit le texte brut original. Cela réduit l'occupation disque de 90% tout en gardant l'expérience acquise. 

4. SCÉNARIOS D'ÉLAGAGE ET CAS PRATIQUES 

Nature du Fragment 

Politique d'Élagage 

Conséquence Systémique 

Axiomes de Sécurité 

Immortels  ( ) 

Jamais supprimés, servent de base de comparaison éternelle. 

Brouillons d'IA Guest 

Flush Aggressif 

Les sorties de Gemini/Claude non validées sont purgées après 24h. 

Code Refactorisé 

Archivage avec Lien 

Les anciennes fonctions sont déplacées en archive froide pour comparaison. 

Préférences Utilisateur 

Renforcement 

Chaque interaction réinitialise le compteur  , rendant l'oubli impossible pour l'intime. 

5. SYNERGIE HARDWARE : L'OUBLI PILOTÉ PAR LA TENSION 

Le Kernel ajuste la constante   en fonction de la télémétrie matérielle (Brique 12). C'est le réflexe de survie de la machine. 

Pression VRAM (RTX 3060) :  Si la mémoire vidéo sature, le système déclenche une &quot;Amnésie de Travail&quot;. Il évince les fragments les moins utiles de la RAM pour laisser de la place au calcul immédiat. 

Pression I/O (Infomaniak 64 vCPUs) :  Si la latence de PostgreSQL augmente, le Kernel force une phase de  Distillation Massive  pour réduire la taille des index et restaurer la fluidité des jointures SQL. 

Abondance NVMe :  Si le stockage est sous-utilisé,   diminue. R2D2 se permet d'être plus &quot;curieux&quot; et de conserver des détails qui pourraient s'avérer utiles plus tard. 

6. INVARIANTS DE SÉCURITÉ ET RÉVERSIBILITÉ 

L'oubli ne doit pas être un acte de destruction aveugle. 

Le &quot;Core Snapshot&quot; :  Avant chaque grande purge circadienne, un instantané compressé de la mémoire est envoyé vers un stockage froid. 

La Trace d'Oubli (Shadow Hash) :  Un fragment supprimé laisse une empreinte cryptographique. Si la même erreur de raisonnement réapparaît, l'IA reconnaît le hash et &quot;ressent&quot; qu'elle a déjà rejeté cette idée par le passé, empêchant les cycles de pensée infinis. 

Veto de l'Expert :  Un agent Senior peut &quot;épingler&quot; (pin) un fragment, lui conférant une immunité temporaire contre l'algorithme ASU, même si son usage est faible. 

7. ANALYSE DES CONSÉQUENCES 

Un élagage trop agressif peut mener à une &quot;IA sans passé&quot;, incapable de comprendre le contexte d'un long projet. Un élagage trop faible mène à une &quot;IA confuse&quot;, noyée dans ses propres itérations. Le réglage de   est donc le paramètre de santé mentale le plus critique du Chef de Forge.