import subprocess
crates = ["r2d2-kernel", "r2d2-mcp", "r2d2-cortex", "r2d2-vision", "r2d2-audio", "r2d2-secure-mem", "r2d2-blackboard", "r2d2-paradox", "r2d2-bitnet", "r2d2-inference-cpu", "r2d2-jsonai"]
bad = []
for c in crates:
    try:
        res = subprocess.run(["cargo", "check", "-p", c], capture_output=True, text=True)
        if res.returncode != 0:
            if "Finished" not in res.stderr and "Finished" not in res.stdout:
                bad.append(c)
                with open(f"{c}_err.txt", "w") as f:
                    f.write(res.stdout + "\n" + res.stderr)
    except Exception as e:
        pass
with open("bad_crates.txt", "w") as f:
    f.write(",".join(bad))
