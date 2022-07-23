import Konva from 'konva';

import { ready } from './common.js';

const IBB_WIDTH = 358;
const IBB_HEIGHT = 123;
const PREVIEW_SCALE_FACTOR = 3; // Preview is scaled with a factor of 3
const MARGIN = 10;

/**
 * Load an SVG file.
 */
async function loadSvg(ev, svg, layer) {
    if (svg.text) {
        const r = await fetch('/preview/', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ svg: svg.text }),
        });
        if (r.ok) {
            const polylines = await r.json();
            layer.destroyChildren();
            drawPreview(layer, polylines);
        } else {
            console.error('Error: HTTP', r.status);
            if (r.status == 400) {
                alert('Error. Did you upload a valid SVG file?');
            } else {
                alert(`Error (HTTP ${r.status})`);
            }
        }
    }
}

// Re-scale group to fit and center it in bounds
function fitGroup(group, bounds, margin) {
    // note: Don't use offset(), as it will change how scaling is
    // applied, which will break going back to the iboardbot

    // Re-scale group to fit
    let clientRect = group.getClientRect({
        skipShadow: true,
        skipStroke: true,
    });

    // scale to fit in bounds minus margin
    const targetSize = {
        width: bounds.width - margin,
        height: bounds.height - margin,
    };
    if (
        clientRect.width / clientRect.height >
        targetSize.width / targetSize.height
    ) {
        group.scale({
            x: targetSize.width / clientRect.width,
            y: targetSize.width / clientRect.width,
        });
    } else {
        group.scale({
            x: targetSize.height / clientRect.height,
            y: targetSize.height / clientRect.height,
        });
    }

    // move group to center of viewport
    clientRect = group.getClientRect({
        skipShadow: true,
        skipStroke: true,
    });
    group.move({
        x: (IBB_WIDTH - clientRect.width) / 2 - clientRect.x,
        y: (IBB_HEIGHT - clientRect.height) / 2 - clientRect.y,
    });
}

function drawPreview(layer, polylines) {
    // Create group of all polylines
    const group = new Konva.Group({
        draggable: true,
        name: 'polylines',
    });
    for (let polyline of polylines) {
        const polylineObj = new Konva.Line({
            points: polyline.map((pair) => [pair.x, pair.y]).flat(),
            stroke: 'black',
            hitStrokeWidth: 40, // make it easier to click for dragging
        });
        group.add(polylineObj);
    }

    fitGroup(group, { width: IBB_WIDTH, height: IBB_HEIGHT }, MARGIN);

    // Add to canvas
    layer.add(group);
}

/**
 * Send the object to the printer.
 */
function printObject(svg, layer) {
    const printMode = document.querySelector('input[name=mode]:checked').value;

    const children = layer.getChildren((node) => node.hasName('polylines'));
    if (children.length == 0) {
        alert('No object loaded. Please choose an SVG file first.');
        return;
    }

    children.forEach(async (obj, i) => {
        console.debug(`Object ${i}:`);
        const dx = obj.x();
        const dy = obj.y();
        console.debug('  Moved by', dx, dy);
        console.debug('  Scaled by', obj.scaleX, obj.scaleY);

        const r = await fetch('/print/', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                svg: svg.text,
                offset_x: dx,
                offset_y: dy,
                scale_x: obj.scaleX(),
                scale_y: obj.scaleY(),
                mode: printMode,
            }),
        });

        if (r.status == 204) {
            // Success TODO
            if (printMode == 'once') {
                alert('Printing!');
            } else {
                alert('Scheduled printing!');
            }
        } else {
            // Error
            console.error('Error: HTTP', r.status);
            if (r.status == 400) {
                alert('Error. Did you upload a valid SVG file?');
            } else {
                alert(`Error (HTTP ${r.status})`);
            }
        }
    });
}

ready(() => {
    console.info('Started.');

    const stage = new Konva.Stage({
        container: 'preview',
        width: IBB_WIDTH * PREVIEW_SCALE_FACTOR,
        height: IBB_HEIGHT * PREVIEW_SCALE_FACTOR,
        scale: { x: PREVIEW_SCALE_FACTOR, y: PREVIEW_SCALE_FACTOR },
    });
    const layer = new Konva.Layer();
    stage.add(layer);

    // Add a transformer for resize handles
    const tr_layer = new Konva.Layer();
    stage.add(tr_layer);
    const tr = new Konva.Transformer({
        rotateEnabled: false,
    });
    tr_layer.add(tr);

    // clicks should select/deselect
    stage.on('click tap', function (e) {
        if (e.target === stage) {
            tr.nodes([]);
        } else if (!tr.nodes().includes(e.target.getParent())) {
            tr.nodes([e.target.getParent()]);
        } else {
            tr.nodes([]);
        }
    });

    let svg = {
        text: '',
    };

    const fileInput = document.querySelector('input[name=file]');
    fileInput.addEventListener('change', (changeEvent) => {
        const file = fileInput.files[0];
        if (file !== undefined) {
            const fr = new FileReader();
            fr.onload = function (ev) {
                svg.text = ev.target.result;
                loadSvg(ev, svg, layer);
            };
            fr.readAsText(file);
        }
    });

    const print = document.querySelector('input#print');
    if (print !== null) {
        print.addEventListener('click', (_clickEvent) =>
            printObject(svg, layer)
        );
    }
});
