import shutil, os, subprocess

dirs = ['r2d2-kernel', 'r2d2-secure-mem', 'r2d2-jsonai', 'r2d2-paradox']
for d in dirs:
    git_path = os.path.join(d, '.git')
    try:
        if os.path.exists(git_path):
            shutil.rmtree(git_path)
            print(f"Removed {git_path}")
    except Exception as e:
        print(f"Error removing {git_path}: {e}")

# Ignore errors if not in cache
subprocess.run(['git', 'rm', '-rf', '--cached'] + dirs)

# Add correct files
res = subprocess.run(['git', 'add'] + dirs)
if res.returncode == 0:
    subprocess.run(['git', 'commit', '-m', 'fix(ci): track source files instead of git submodules'])
    print("Fixed and committed!")
else:
    print("Git add failed")
