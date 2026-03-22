import subprocess
with open("python_out.txt", "w") as f:
    result = subprocess.run(["cargo", "check", "--workspace"], capture_output=True, text=True)
    f.write(result.stdout)
    f.write("\n--STDERR--\n")
    f.write(result.stderr)
