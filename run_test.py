import subprocess
import time

print("Demarrage du Serveur MCP Cargo en arriere-plan...")

# Charger le payload exact
with open('test_audio.json', 'r') as f:
    data = f.read()

# Lancer R2D2-MCP
p = subprocess.Popen(
    ['cargo', 'run', '--bin', 'r2d2-mcp'],
    stdin=subprocess.PIPE,
    stdout=open('mcp_out.log', 'w'),
    stderr=subprocess.STDOUT
)

print("Injection du Payload RPC (Ingest Audio)...")
p.stdin.write(data.encode('utf-8'))
p.stdin.write(b'\n')
p.stdin.flush()

print("Attente de 60 secondes pour le telechargement de Whisper et l'inference...")
time.sleep(60)

print("Terminaison du processus.")
p.terminate()
