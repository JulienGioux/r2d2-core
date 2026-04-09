import puppeteer from 'puppeteer-core';
import fs from 'fs';

async function main() {
    try {
        const browser = await puppeteer.connect({ browserURL: 'http://127.0.0.1:9222', defaultViewport: null });
        const pages = await browser.pages();
        
        // Trouver la page NotebookLM active
        let targetPage = null;
        for (const p of pages) {
            const url = p.url();
            if (url.includes('notebooklm.google.com/notebook/')) {
                targetPage = p;
                break;
            }
        }

        if (!targetPage) {
            console.error("PAGE NON TROUVEE");
            process.exit(1);
        }

        const domStructure = await targetPage.evaluate(() => {
            // Identifier tous les éléments qui ressemblent à des messages ou du texte
            const elements = Array.from(document.querySelectorAll('div, role, aria-live'));
            let results = [];
            
            // On s'intéresse aux bulles de messagerie
            const messages = document.querySelectorAll('*[role="log"], *[aria-live="polite"], chat-message, .message, .response, spark-message, model-response');
            
            const rawBody = document.body.innerHTML;

            return {
                messagesCount: messages.length,
                messagesHTML: Array.from(messages).map(el => ({
                    tag: el.tagName,
                    id: el.id,
                    className: el.className,
                    role: el.getAttribute('role'),
                    ariaLive: el.getAttribute('aria-live'),
                    textFragment: el.innerText.substring(0, 100)
                })),
                rawBodyFragment: rawBody.substring(rawBody.length - 2000) // 2000 derniers charactères
            };
        });

        fs.writeFileSync('/tmp/notebooklm_dom.json', JSON.stringify(domStructure, null, 2));
        console.log("DOM DUMPED TO /tmp/notebooklm_dom.json");

    } catch(e) {
        console.error(e.message);
    }
    process.exit(0);
}

main();
