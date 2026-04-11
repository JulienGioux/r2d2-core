const { spawn } = require('child_process');

const server = spawn('cargo', ['run', '--release', '-p', 'r2d2-vampire', '--', '--mode', 'json']);

let requestId = 1;

server.stdout.on('data', (data) => {
    const responses = data.toString().split('\n').filter(l => l.trim() !== '');
    for (const response of responses) {
        try {
            const jsonParams = JSON.parse(response);
            if (jsonParams.id) {
                console.log("RÉPONSE MCP :", JSON.stringify(jsonParams, null, 2));
                server.kill();
                process.exit(0);
            }
        } catch (e) {
            // Ignore non-JSON output (les logs passent par stderr)
        }
    }
});

server.stderr.on('data', (data) => {
    process.stderr.write(`STDERR: ${data}`);
});

server.on('close', (code) => {
    console.log(`Processus MCP terminé avec le code ${code}`);
});

const callForge = () => {
    const rawReq = {
        jsonrpc: "2.0",
        id: requestId++,
        method: "tools/call",
        params: {
            name: "mcp_vampire_lord_forge_expert",
            arguments: {
                topic: "RustArch",
                deep_search_queries: [
                    "Reference documentation, official man pages, and low-level technical specifications for the latest Rust language versions and its core ecosystem. Focus on advanced memory management (lifetimes, unsafe Rust, allocators), async/await internals, and advanced macro programming.",
                    "Advanced software architecture and engineering in Rust for critical systems. High-assurance systems, military-grade security standards (MIL-SPEC), zero-trust networking, cryptography implementations, and robust Hexagonal (Ports & Adapters) design patterns.",
                    "Low-level system programming and embedded (no_std) development in Rust. Hardware interfacing, memory-mapped I/O, writing device drivers, kernel modules, real-time operating system (RTOS) concepts, and hardware-compliant execution for scientific/industrial environments."
                ]
            }
        }
    };
    const payload = JSON.stringify(rawReq) + "\n";
    console.log("Envoi de la requête de Forge...");
    server.stdin.write(payload);
};

// Attendre 2 secondes que le serveur initialise ses routes
setTimeout(callForge, 2000);
