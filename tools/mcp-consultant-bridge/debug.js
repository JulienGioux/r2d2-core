import puppeteer from 'puppeteer-core';
import fs from 'fs';

async function main() {
    try {
        const browser = await puppeteer.connect({ browserURL: 'http://127.0.0.1:9222', defaultViewport: null });
        const pages = await browser.pages();
        let targetPage = pages.find(p => p.url().includes('notebooklm.google.com/notebook/'));

        if (!targetPage) {
            console.log("No notebookLM page found.");
            process.exit(1);
        }

        const html = await targetPage.evaluate(() => {
            // Remplacer le contenu énorme par juste l'arbre de chat
            const body = document.body;
            // Ne garder que le HTML (les classes, tags) sans le SVG polluant
            const clone = body.cloneNode(true);
            clone.querySelectorAll('svg, path, script, style, link').forEach(e => e.remove());
            return clone.innerHTML;
        });

        fs.writeFileSync('/tmp/dom.html', html);
        console.log("DOM DUMPED TO /tmp/dom.html");

    } catch(e) {
        console.error("Error: ", e.message);
    }
    process.exit(0);
}

main();
