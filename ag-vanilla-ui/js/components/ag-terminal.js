class AgTerminal extends HTMLElement {
    constructor() {
        super();
        this.attachShadow({ mode: 'open' });
        this.term = null;
        this.socket = null;
    }
    
    connectedCallback() {
        // Injection du style xterm localement dans le shadow DOM
        this.shadowRoot.innerHTML = `
            <style>
                @import url('https://cdn.jsdelivr.net/npm/xterm@5.3.0/css/xterm.min.css');
                :host {
                    display: block;
                    width: 100%;
                    height: 100%;
                    background: #000;
                    padding: 8px;
                    box-sizing: border-box;
                }
                #terminal-container { width: 100%; height: 100%; }
                
                /* Custom scrollbar for terminal */
                #terminal-container { scrollbar-width: thin; scrollbar-color: #333 #000; }
                ::-webkit-scrollbar { width: 8px; }
                ::-webkit-scrollbar-track { background: #000; }
                ::-webkit-scrollbar-thumb { background: #333; border-radius: 4px; }
            </style>
            <div id="terminal-container"></div>
        `;
        
        // Dynamic loading of XTerm script into the main document if not present
        if (!window.Terminal) {
            const script = document.createElement('script');
            script.src = "https://cdn.jsdelivr.net/npm/xterm@5.3.0/lib/xterm.min.js";
            script.onload = () => this.initTerminal();
            document.head.appendChild(script);
        } else {
            this.initTerminal();
        }
    }
    
    initTerminal() {
        this.term = new window.Terminal({
            fontFamily: '"JetBrains Mono", monospace',
            fontSize: 13,
            theme: { background: '#050505', cursor: '#E5E7EB', selectionBackground: 'rgba(255,255,255,0.2)' },
            cursorBlink: true,
            convertEol: true // Automatically convert CR to CRLF
        });
        
        const container = this.shadowRoot.getElementById('terminal-container');
        this.term.open(container);
        this.term.writeln('\x1b[38;2;16;185;129m[AG-Vanilla]\x1b[0m Booting Sovereign PTY connection...');
        
        const host = window.location.host;
        const sessionId = this.getAttribute('data-session-id') || document.querySelector('meta[name="session-id"]')?.content;
        
        let protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        this.socket = new WebSocket(`${protocol}//${host}/api/ws/terminal/${sessionId}`);
        
        this.socket.binaryType = 'arraybuffer';
        
        this.socket.onopen = () => {
            this.term.writeln('\x1b[38;2;16;185;129m[AG-Vanilla]\x1b[0m Connection established.');
        };
        
        this.socket.onmessage = (event) => {
            if (typeof event.data === 'string') {
                this.term.write(event.data);
            } else {
                // ArrayBuffer -> Uint8Array
                this.term.write(new Uint8Array(event.data));
            }
        };
        
        this.term.onData((data) => {
            if (this.socket.readyState === WebSocket.OPEN) {
                this.socket.send(data);
            }
        });
        
        this.socket.onclose = () => {
            this.term.writeln('\r\n\x1b[31;1m[ERREUR: Session Términée]\x1b[0m');
            this.term.writeln('\x1b[38;5;244mImpossible de maintenir la connexion PTY.\x1b[0m');
            this.term.writeln('\x1b[38;5;244mLe conteneur environnement "r2d2-workspace" est probablement éteint ou injoignable.\x1b[0m');
        };
        
        // Window resize observer
        new ResizeObserver(() => {
            if(window.FitAddon) {
                // Feature for next iterations
            }
        }).observe(container);
    }
    
    disconnectedCallback() {
        if (this.socket) {
            this.socket.close();
        }
        if (this.term) {
            this.term.dispose();
        }
    }
}
customElements.define('ag-terminal', AgTerminal);
