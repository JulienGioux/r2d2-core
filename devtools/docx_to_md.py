#!/usr/bin/env python3
"""
################################################################################
# R2D2 FORGE - DEVTOOLS: Universal DOCX to Markdown Converter
################################################################################
#
# MISSION :
# Outil générique pour parser des documents Microsoft Word (.docx) et les 
# transpiler en Markdown structuré pur. Conçu pour bypasser les lecteurs PDF/Word
# et ingérer directement la connaissance dans des vector databases (Blackboard).
#
# UTILISATION :
# Python: python3 docx_to_md.py --input /dossier/source --output /dossier/cible
#
# ARCHITECTURE (FALLBACK INTELLIGENT) :
# - Tente d'utiliser `mammoth` et `markdownify` pour une extraction HTML->MD parfaite (images, tableaux).
# - En cas d'absence des dépendances (ex: environnement WSL verrouillé), bascule 
#   automatiquement sur un moteur PURE PYTHON (xml.etree) 0 dépendance.
################################################################################
"""

import os
import glob
import argparse
import sys

# Tentative d'importation des dépendances premium
try:
    import mammoth
    from markdownify import markdownify as md
    HAS_MAMMOTH = True
except ImportError:
    HAS_MAMMOTH = False
    import zipfile
    import xml.etree.ElementTree as ET

def convert_premium(docx_path: str, md_path: str) -> bool:
    """Conversion Haute Fidélité via Mammoth (Préserve Tableaux & Styles Complexes)."""
    try:
        with open(docx_path, "rb") as docx_file:
            result = mammoth.convert_to_html(docx_file)
            markdown_content = md(result.value, heading_style="ATX")
            with open(md_path, "w", encoding="utf-8") as md_file:
                md_file.write(markdown_content)
            return True
    except Exception as e:
        print(f"❌ Erreur Mammoth sur {docx_path}: {e}")
        return False

def convert_pure_python(docx_path: str, md_path: str) -> bool:
    """Conversion Pure Python (Fallback) basée sur l'extraction directe de l'arbre OOXML."""
    try:
        namespaces = {'w': 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'}
        md_lines = []
        with zipfile.ZipFile(docx_path) as docx:
            xml_content = docx.read('word/document.xml')
            root = ET.fromstring(xml_content)
            body = root.find('w:body', namespaces)
            if body is None: return False
            
            for paragraph in body.findall('.//w:p', namespaces):
                pPr = paragraph.find('w:pPr', namespaces)
                heading_level, is_list = 0, False
                if pPr is not None:
                    pStyle = pPr.find('w:pStyle', namespaces)
                    if pStyle is not None:
                        val = pStyle.get('{http://schemas.openxmlformats.org/wordprocessingml/2006/main}val', '')
                        if 'Heading' in val or 'Titre' in val:
                            digits = [int(s) for s in val if s.isdigit()]
                            heading_level = digits[0] if digits else 1
                    if pPr.find('w:numPr', namespaces) is not None:
                        is_list = True
                
                texts = [t.text for run in paragraph.findall('.//w:r', namespaces) 
                         if (t := run.find('w:t', namespaces)) is not None and t.text]
                line_text = "".join(texts).strip()
                if not line_text: continue
                
                if heading_level > 0: md_lines.append(f"\n{'#' * heading_level} {line_text}\n")
                elif is_list: md_lines.append(f"- {line_text}")
                else: md_lines.append(f"{line_text}\n")
                
        with open(md_path, "w", encoding="utf-8") as f:
            f.write("\n".join(md_lines))
        return True
    except Exception as e:
        print(f"❌ Erreur Fallback sur {docx_path}: {e}")
        return False

def main():
    parser = argparse.ArgumentParser(description="Extracteur R2D2: DOCX -> Markdown")
    parser.add_argument("-i", "--input", required=True, help="Dossier source (.docx)")
    parser.add_argument("-o", "--output", required=True, help="Dossier cible (.md)")
    args = parser.parse_args()

    os.makedirs(args.output, exist_ok=True)
    files = glob.glob(os.path.join(args.input, "*.docx"))
    
    print(f"🚀 Moteur: {'Prestige (Mammoth)' if HAS_MAMMOTH else 'Pure Python (Fallback)'} | Cibles: {len(files)}")
    
    for f in files:
        base = os.path.splitext(os.path.basename(f))[0]
        md_path = os.path.join(args.output, f"{base}.md")
        success = convert_premium(f, md_path) if HAS_MAMMOTH else convert_pure_python(f, md_path)
        if success: print(f"✔️ {base}.md")

if __name__ == "__main__":
    main()
