import subprocess
with open("test_output.log", "w") as f:
    try:
        output = subprocess.check_output(["cargo", "test", "--workspace"], stderr=subprocess.STDOUT)
        f.write(output.decode())
    except subprocess.CalledProcessError as e:
        f.write(e.output.decode())
        f.write("\nFAILED")
