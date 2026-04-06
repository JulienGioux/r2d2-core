// AG-Vanilla Sovereign UI Framework 
// Version: 1.0.0
// Description: Framework CSS/JS natif, Zéro-Dépendance, distribué en composants Vanilla.

console.log("%c[AG-Vanilla] System Initialized", "color: #10b981; font-weight: bold; background: #000; padding: 2px 6px; border-radius: 4px;");

const AgVanilla = {
    // API de création de fenêtres virtuelles (Mock before Xterm WebSocket)
    spawnTerminal: function() {
        const desktop = document.getElementById('desktop-environment');
        if (!desktop) return;
        
        const count = desktop.querySelectorAll('ag-window').length;
        const newWin = document.createElement('ag-window');
        newWin.setAttribute('title', `tty-vtty${count+1}`);
        
        const offset = (count * 30) % 200;
        newWin.style.top = `${150 + offset}px`;
        newWin.style.left = `${200 + offset}px`;
        newWin.style.width = '640px';
        newWin.style.height = '400px';
        
        newWin.innerHTML = `
            <ag-terminal></ag-terminal>
        `;
        desktop.appendChild(newWin);
    },
    
    // Abstracting icon injection
    initIcons: function() {
        if (typeof lucide !== 'undefined') {
            lucide.createIcons();
        }
    }
};

window.AgVanilla = AgVanilla;

// Initialization when the DOM is ready
document.addEventListener('DOMContentLoaded', () => {
    AgVanilla.initIcons();
});

// HTMX Integration: Re-render icons seamlessly when new templates are injected into the DOM
document.addEventListener('htmx:afterSettle', () => {
    AgVanilla.initIcons();
});
