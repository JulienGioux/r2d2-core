#!/usr/bin/env python3
"""
################################################################################
# R2D2 FORGE - DEVTOOLS: Universal DOCX to Markdown Converter
################################################################################
#
# MISSION :
# Convertir des lots de fichiers Microsoft Word (.docx) en fichiers Markdown (.md)
# stricts et lisibles. Préserve les tableaux, les listes, et la hiérarchie H1-H6.
#
# APPROCHE TECHNIQUE :
# 1. Utilisation de la librairie `mammoth` pour extraire la sémantique HTML du DOCX.
#    (Mammoth se concentre sur la sémantique plutôt que le style pur pour éviter le bruit).
# 2. Utilisation de `markdownify` pour projeter le HTML en Markdown propre (ATX).
#
# UTILISATION GÉNÉRIQUE :
#   python3 docx_to_md.py --input /chemin/vers/docx --output /chemin/vers/md
#
# SÉCURITÉ & RÉSILIENCE :
# - Gère silencieusement les fichiers corrompus.
# - Crée le dossier de destination s'il n'existe pas.
################################################################################
"""

import os
import glob
import argparse
import sys
import subprocess

# Auto-Bootstrapping: Installation silencieuse des dépendances si absentes
try:
    import mammoth
    from markdownify import markdownify as md
except ImportError:
    print("⏳ [Bootstrapper] Installation autonome des dépendances (mammoth, markdownify)...")
    subprocess.check_call([sys.executable, "-m", "pip", "install", "mammoth", "markdownify", "--quiet"])
    import mammoth
    from markdownify import markdownify as md

def convert_single_file(docx_path: str, output_dir: str) -> bool:
    """
    Convertit un unique fichier DOCX vers son équivalent Markdown.
    Retourne True en cas de succès, False sinon.
    """
    filename = os.path.basename(docx_path)
    base_name = os.path.splitext(filename)[0]
    md_path = os.path.join(output_dir, f"{base_name}.md")
    
    try:
        with open(docx_path, "rb") as docx_file:
            # 1. Extraction Sémantique -> HTML Strict
            result = mammoth.convert_to_html(docx_file)
            html = result.value
            
            # 2. Transpilation HTML -> Markdown
            markdown_content = md(html, heading_style="ATX", bullet_list_marker="-")
            
            # 3. Persistance
            with open(md_path, "w", encoding="utf-8") as md_file:
                md_file.write(markdown_content)
                
            return True
    except Exception as e:
        print(f"❌ [Erreur] Échec sur {filename}: {str(e)}")
        return False

def main():
    parser = argparse.ArgumentParser(description="Extracteur R2D2: DOCX -> Markdown")
    parser.add_argument("-i", "--input", required=True, help="Dossier contenant les .docx")
    parser.add_argument("-o", "--output", required=True, help="Dossier cible pour les .md")
    args = parser.parse_args()

    input_dir = os.path.abspath(args.input)
    output_dir = os.path.abspath(args.output)

    if not os.path.exists(input_dir):
        print(f"🚨 [Fatal] Le dossier source n'existe pas : {input_dir}")
        sys.exit(1)

    if not os.path.exists(output_dir):
        print(f"📁 [Système] Création du dossier cible : {output_dir}")
        os.makedirs(output_dir)

    search_pattern = os.path.join(input_dir, "*.docx")
    docx_files = glob.glob(search_pattern)
    
    if not docx_files:
        print(f"⚠️ [Attention] Aucun fichier .docx trouvé dans {input_dir}")
        sys.exit(0)

    print(f"🚀 [Pipeline] Début de la conversion massive de {len(docx_files)} fichiers...")
    
    success_count = 0
    for file_path in docx_files:
        if convert_single_file(file_path, output_dir):
            success_count += 1
            print(f"✔️ O.K. : {os.path.basename(file_path)}")

    print(f"\n✅ [Terminé] Taux de réussite : {success_count}/{len(docx_files)}")

if __name__ == "__main__":
    main()
