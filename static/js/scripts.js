const IBB_WIDTH = 358;
const IBB_HEIGHT = 123;
const PREVIEW_SCALE_FACTOR = 3; // Preview is scaled with a factor of 3

/**
 * Load an SVG file.
 */
async function loadSvg(ev, svg, canvas) {
    if (svg.text) {
        const r = await fetch('/preview/', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ svg: svg.text }),
        });
        if (r.ok) {
            const polylines = await r.json();
            canvas.clear();
            drawPreview(canvas, polylines);
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

/**
 * Scale and transform polyline so it can be used by fabric.js.
 */
function preparePolyline(polyline, scaleFactor) {
    return polyline.map((pair) => ({
        x: pair.x * scaleFactor,
        y: pair.y * scaleFactor,
    }));
}

function drawPreview(canvas, polylines) {
    // Create group of all polylines
    const group = [];
    for (let polyline of polylines) {
        const polylineObj = new fabric.Polyline(
            preparePolyline(polyline, PREVIEW_SCALE_FACTOR),
            {
                stroke: 'black',
                fill: null,
                lockUniScaling: true,
                lockRotation: true,
            }
        );
        group.push(polylineObj);
    }
    const groupObj = new fabric.Group(group);

    // Re-scale group to fit and center it in viewport
    const offset = 5 * PREVIEW_SCALE_FACTOR;
    const height = IBB_HEIGHT * PREVIEW_SCALE_FACTOR;
    const width = IBB_WIDTH * PREVIEW_SCALE_FACTOR;
    if (groupObj.height / groupObj.width > height / width) {
        groupObj.scaleToHeight(height - offset * 2);
    } else {
        groupObj.scaleToWidth(width - offset * 2);
    }
    const centerpoint = new fabric.Point(width / 2, offset);
    groupObj.setPositionByOrigin(centerpoint, 'center', 'top');

    // Add to canvas
    canvas.add(groupObj);
}

/**
 * Send the object to the printer.
 */
function printObject(svg, canvas) {
    return function (clickEvent) {
        const printMode = document.querySelector(
            'input[name=mode]:checked'
        ).value;

        if (canvas.getObjects().length == 0) {
            alert('No object loaded. Please choose an SVG file first.');
            return;
        }

        canvas.forEachObject(async (obj, i) => {
            console.debug(`Object ${i}:`);
            const dx = (obj.left - obj._originalLeft) / PREVIEW_SCALE_FACTOR;
            const dy = (obj.top - obj._originalTop) / PREVIEW_SCALE_FACTOR;
            console.debug('  Moved by', dx, dy);
            console.debug('  Scaled by', obj.scaleX, obj.scaleY);

            const r = await fetch('/print/', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    svg: svg.text,
                    offset_x: dx,
                    offset_y: dy,
                    scale_x: obj.scaleX,
                    scale_y: obj.scaleY,
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
    };
}

ready(() => {
    console.info('Started.');

    // Fabric.js canvas object
    const canvas = new fabric.Canvas('preview');
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
                loadSvg.bind(this)(ev, svg, canvas);
            };
            fr.readAsText(file);
        }
    });

    const print = document.querySelector('input#print');
    print.addEventListener('click', printObject(svg, canvas));
});
