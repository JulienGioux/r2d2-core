#!/usr/bin/env python3
"""
################################################################################
# R2D2 FORGE - DEVTOOLS: Universal Blackboard Importer (JSON-RPC 2.0)
################################################################################
#
# MISSION :
# Injecter massivement un répertoire de fichiers texte ou markdown locaux 
# vers un Gateway MCP hébergeant la base de données vectorielle (Paradox Blackboard).
#
# UTILISATION GÉNÉRIQUE :
# Python: python3 blackboard_ingest.py --dir /chemin/vers/md --url http://127.0.0.1:3030/mcp
#
# ARGUMENTS AVANCÉS :
# --batch-size : Nombre de documents par payload JSON-RPC (Défaut: 5)
# --mime-type  : Type sémantique des documents (Défaut: "text/markdown")
################################################################################
"""

import os
import glob
import json
import urllib.request
import argparse
import sys

def chunk_list(lst, n):
    for i in range(0, len(lst), n):
        yield lst[i:i + n]

def build_rpc_payload(files_chunk, mime_type="text/markdown", batch_id=0):
    """Construit un payload RPC batch conforme au standard R2D2-MCP."""
    payloads = []
    for i, file_path in enumerate(files_chunk):
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
            
        payloads.append({
            "jsonrpc": "2.0",
            "method": "store_memory",
            "params": {
                "content": content,
                "mime_type": mime_type,
                "metadata": {
                    "source_file": os.path.basename(file_path),
                    "ingestion_agent": "Antigravity_DevTools",
                    "batch_id": str(batch_id)
                }
            },
            "id": f"ingest_{batch_id}_{i}"
        })
    return payloads

def main():
    parser = argparse.ArgumentParser(description="Injecteur R2D2 Blackboard")
    parser.add_argument("-d", "--dir", required=True, help="Dossier contenant les documents")
    parser.add_argument("-u", "--url", default="http://127.0.0.1:3030/mcp", help="URL du noeud MCP")
    parser.add_argument("-b", "--batch-size", default=10, type=int, help="Taille des paquets réseau")
    parser.add_argument("-e", "--extension", default="*.md", help="Filtre glob (ex: *.md, *.txt)")
    args = parser.parse_args()

    target_files = glob.glob(os.path.join(args.dir, args.extension))
    if not target_files:
        print(f"⚠️ Aucun fichier '{args.extension}' trouvé dans {args.dir}")
        sys.exit(0)

    print(f"🚀 Début de l'ingestion de {len(target_files)} fragments vers {args.url} (Batch: {args.batch_size})")

    success_total = 0
    for batch_id, chunk in enumerate(chunk_list(target_files, args.batch_size)):
        payload = build_rpc_payload(chunk, batch_id=batch_id)
        
        req = urllib.request.Request(
            url=args.url,
            data=json.dumps(payload).encode('utf-8'),
            headers={'Content-Type': 'application/json'},
            method='POST'
        )
        
        try:
            with urllib.request.urlopen(req) as response:
                if response.status == 200:
                    print(f"✔️ Batch {batch_id} injecté ({len(chunk)} docs)")
                    success_total += len(chunk)
                else:
                    print(f"❌ Batch {batch_id} rejeté. Statut: {response.status}")
        except Exception as e:
            print(f"❌ Erreur réseau sur Batch {batch_id}: {e}")

    print(f"\n✅ Ingestion complétée : {success_total}/{len(target_files)} documents stockés.")

if __name__ == "__main__":
    main()
