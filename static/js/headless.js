import { ready } from './common';

/**
 * Load the list of SVG files.
 */
async function loadSvgList() {
    const r = await fetch('/list/', {
        headers: { 'Content-Type': 'application/json' },
    });
    if (r.ok) {
        const files = await r.json();
        console.log('Loaded list of SVG files');
        const element = document.querySelector('#svgfiles');
        // Hide loading text
        element.querySelector('.loading').hidden = true;
        // Show files
        if (files.length === 0) {
            element.querySelector('.nofiles').hidden = false;
        } else {
            const list = element.querySelector('ul.files');
            for (const file of files) {
                const entry = document.createElement('li');
                entry.appendChild(document.createTextNode(file));
                list.appendChild(entry);
            }
            list.hidden = false;
        }
    } else {
        console.error('Error: HTTP', r.status);
        const element = document.querySelector('#svgfiles');
        // Hide loading text
        element.querySelector('.loading').hidden = true;
        // Show error
        const error = element.querySelector('.error');
        error.innerText = `Error fetching SVG files (HTTP ${r.status})`;
        try {
            const parsedResponse = await r.json();
            error.innerText += '\nDetails: ' + parsedResponse.details;
        } catch {}
        error.hidden = false;
    }
}

/**
 * Load the configuration.
 */
async function loadConfig() {
    const r = await fetch('/config/', {
        headers: { 'Content-Type': 'application/json' },
    });
    if (r.ok) {
        const config = await r.json();
        console.log('Loaded config');
        const element = document.querySelector('#config');
        // Hide loading text
        element.querySelector('.loading').hidden = true;
        // Show config
        const items = element.querySelector('dl.items');
        const configEntries = [
            { key: 'device', label: 'Device' },
            { key: 'svg_dir', label: 'SVG Directory' },
            {
                key: 'interval_seconds',
                label: 'Start drawing every n seconds',
            },
        ];
        for (const item of configEntries) {
            const key = document.createElement('dt');
            key.appendChild(document.createTextNode(item.label));
            items.appendChild(key);
            const value = document.createElement('dd');
            const valueCode = document.createElement('code');
            valueCode.appendChild(document.createTextNode(config[item.key]));
            value.appendChild(valueCode);
            items.appendChild(value);
        }
        items.hidden = false;
    } else {
        console.error('Error: HTTP', r.status);
        const element = document.querySelector('#config');
        // Hide loading text
        element.querySelector('.loading').hidden = true;
        // Show error
        const error = element.querySelector('.error');
        error.innerText = `Error fetching config (HTTP ${r.status})`;
        error.hidden = false;
    }
}

ready(() => {
    console.info('Started.');

    loadConfig();
    loadSvgList();
});
