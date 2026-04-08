import puppeteer from 'puppeteer-core';
import fs from 'fs';

(async () => {
    let browser;
    try {
        browser = await puppeteer.connect({ browserURL: 'http://127.0.0.1:9222', defaultViewport: null });
        console.log("Connected to browser");
        const page = await browser.newPage();
        await page.goto("https://notebooklm.google.com/notebook/4dd65131-ea87-47a3-8958-a647351c4050", { waitUntil: 'networkidle2' });
        
        console.log("Page loaded");
        // We will output elements that look like chat input directly to stdout
        const inputNodes = await page.evaluate(() => {
            const textareas = Array.from(document.querySelectorAll('textarea'));
            return textareas.map(t => ({
                id: t.id,
                className: t.className,
                placeholder: t.placeholder,
                ariaLabel: t.getAttribute('aria-label')
            }));
        });
        
        console.log("INPUT NODES FOUND:", JSON.stringify(inputNodes, null, 2));

        const messages = await page.evaluate(() => {
             // Find div containing messages
             // NotebookLM uses .chat-message or similar. We will just dump all text content inside the main chat view
             const chatNodes = Array.from(document.querySelectorAll('[role="log"], .chat-container, .document-content'));
             if (chatNodes.length > 0) return chatNodes[0].innerText.substring(0, 500);
             return "No log or chat-container found";
        });

        console.log("CHAT PREVIEW:", messages);

    } catch (e) {
        console.error("Probe Error:", e);
    } finally {
        if (browser) browser.disconnect();
        process.exit(0);
    }
})();
