const { spawn } = require('child_process');

const server = spawn('cargo', ['run', '--release', '-p', 'r2d2-vampire', '--', '--mode', 'json']);

let requestId = 1;
server.stdout.on('data', (data) => {
    const responses = data.toString().split('\n').filter(l => l.trim() !== '');
    for (const response of responses) {
        try {
            const jsonParams = JSON.parse(response);
            if (jsonParams.id) {
                console.log("RÉPONSE NotebookLM via MCP :\n", JSON.stringify(jsonParams, null, 2));
                server.kill();
                process.exit(0);
            }
        } catch (e) { }
    }
});
server.stderr.on('data', (data) => { process.stderr.write(`STDERR: ${data}`); });

setTimeout(() => {
    const req = {
        jsonrpc: "2.0",
        id: requestId++,
        method: "tools/call",
        params: {
            name: "ask_rustymaster",
            arguments: { prompt: "Que penses-tu du projet R2D2 dans sa globalité ? Sois franc." }
        }
    };
    console.log("Transmission de la question à NotebookLM...");
    server.stdin.write(JSON.stringify(req) + "\n");
}, 2000);
