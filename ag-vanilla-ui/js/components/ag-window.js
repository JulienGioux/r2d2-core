class AgWindow extends HTMLElement {
    constructor() {
        super();
        this.attachShadow({ mode: 'open' });
        this.isDragging = false;
        this.dragStartX = 0;
        this.dragStartY = 0;
        
        this.onMouseDown = this.onMouseDown.bind(this);
        this.onMouseMove = this.onMouseMove.bind(this);
        this.onMouseUp = this.onMouseUp.bind(this);
    }

    connectedCallback() {
        const title = this.getAttribute('title') || 'ag-window';
        this.shadowRoot.innerHTML = `
            <style>
                :host {
                    position: absolute;
                    display: flex; flex-direction: column;
                    background: rgba(10, 10, 10, 0.85); backdrop-filter: blur(20px);
                    -webkit-backdrop-filter: blur(20px);
                    border: 1px solid rgba(255,255,255,0.1);
                    border-top: 1px solid rgba(255,255,255,0.2);
                    border-radius: 8px;
                    overflow: hidden;
                    box-shadow: 0 12px 32px rgba(0,0,0,0.8);
                    min-width: 300px;
                    min-height: 200px;
                    user-select: none; /* Prevents text selection while dragging */
                }
                .titlebar {
                    height: 32px; background: rgba(0, 0, 0, 0.4);
                    display: flex; align-items: center; justify-content: space-between;
                    padding: 0 12px; cursor: grab;
                    font-family: 'JetBrains Mono', monospace; font-size: 12px; color: #ccc;
                    border-bottom: 1px solid rgba(255,255,255,0.05);
                }
                .titlebar:active { cursor: grabbing; }
                .window-controls { display: flex; gap: 6px; }
                .control-dot { width: 10px; height: 10px; border-radius: 50%; background: #444; transition: background 0.2s;}
                .control-dot.close { background: #ef4444; cursor: pointer; }
                .control-dot.close:hover { filter: brightness(1.2); }
                .control-dot.min { background: #f59e0b; cursor: pointer; }
                .control-dot.min:hover { filter: brightness(1.2); }
                .control-dot.max { background: #10b981; cursor: pointer; }
                .control-dot.max:hover { filter: brightness(1.2); }
                
                .content { 
                    flex: 1; position: relative; overflow: auto; 
                    background: transparent; cursor: default; user-select: text; 
                }
            </style>
            <div class="titlebar" id="drag-handle">
                <span>${title}</span>
                <div class="window-controls">
                    <div class="control-dot min"></div>
                    <div class="control-dot max"></div>
                    <div class="control-dot close" id="close-btn"></div>
                </div>
            </div>
            <div class="content"><slot></slot></div>
        `;

        this.handle = this.shadowRoot.getElementById('drag-handle');
        this.handle.addEventListener('mousedown', this.onMouseDown);
        
        // Close button logic
        const closeBtn = this.shadowRoot.getElementById('close-btn');
        closeBtn.addEventListener('click', () => {
            this.remove();
        });

        // Bring to front on click
        this.addEventListener('mousedown', () => {
            this.bringToFront();
        });
    }

    bringToFront() {
        const windows = document.querySelectorAll('ag-window');
        let maxZ = 100;
        windows.forEach(w => {
            let z = parseInt(window.getComputedStyle(w).zIndex) || 100;
            if (z > maxZ) maxZ = z;
        });
        this.style.zIndex = maxZ + 1;
    }

    onMouseDown(e) {
        if (e.target.classList && e.target.classList.contains('control-dot')) return;
        this.isDragging = true;
        this.bringToFront();
        this.dragStartX = e.clientX - this.offsetLeft;
        this.dragStartY = e.clientY - this.offsetTop;
        
        document.addEventListener('mousemove', this.onMouseMove);
        document.addEventListener('mouseup', this.onMouseUp);
    }

    onMouseMove(e) {
        if (!this.isDragging) return;
        
        let newX = e.clientX - this.dragStartX;
        let newY = e.clientY - this.dragStartY;
        
        if (newY < 0) newY = 0; // Prevent dragging out of top viewport
        
        this.style.left = `${newX}px`;
        this.style.top = `${newY}px`;
    }

    onMouseUp() {
        this.isDragging = false;
        document.removeEventListener('mousemove', this.onMouseMove);
        document.removeEventListener('mouseup', this.onMouseUp);
    }
}
customElements.define('ag-window', AgWindow);
