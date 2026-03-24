import subprocess
with open("python_out.txt", "w", encoding="utf-8") as f:
    try:
        result = subprocess.run(["cargo", "clippy", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"], capture_output=True, text=True, check=False)
        f.write(result.stdout)
        f.write("\n--STDERR--\n")
        f.write(result.stderr)
        f.write(f"\nExit code: {result.returncode}")
    except Exception as e:
        f.write(f"Exception: {e}")
