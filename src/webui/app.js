let key = localStorage.getItem('grid_key');
if (!key) {
    // Create a modal form for grid key entry
    const modal = document.createElement('div');
    modal.style.position = 'fixed';
    modal.style.top = '0';
    modal.style.left = '0';
    modal.style.width = '100vw';
    modal.style.height = '100vh';
    modal.style.background = 'rgba(0,0,0,0.5)';
    modal.style.display = 'flex';
    modal.style.alignItems = 'center';
    modal.style.justifyContent = 'center';
    modal.style.zIndex = '1000';

    const form = document.createElement('form');
    form.style.background = '#23272f';
    form.style.padding = '2em';
    form.style.borderRadius = '8px';
    form.style.boxShadow = '0 2px 8px rgba(0,0,0,0.2)';
    form.innerHTML = `
        <label for="grid-key-input">Enter grid key:</label><br>
        <input type="password" id="grid-key-input" name="grid-key" autocomplete="off" required style="margin-top:0.5em;"><br><br>
        <button type="submit">Submit</button>
    `;

    form.onsubmit = function(e) {
        e.preventDefault();
        const input = form.querySelector('#grid-key-input');
        if (input.value) {
            localStorage.setItem('grid_key', input.value);
            modal.remove();
            location.reload();
        }
    };

    modal.appendChild(form);
    document.body.appendChild(modal);
}

document.getElementById('change-key').onclick = function() {
    localStorage.removeItem('grid_key');
    location.reload();
};

function fetchGrid() {
    fetch(`/grid/${key}`)
        .then(r => r.json())
        .then(data => {
            document.getElementById('status').textContent = `Alive: ${data.alive_nodes}, Dead: ${data.dead_nodes}, Dying: ${data.dying_nodes}, Total: ${data.total_nodes}`;
            const tbody = document.querySelector('#nodes tbody');
            tbody.innerHTML = '';
            data.nodes.forEach(node => {
                const tr = document.createElement('tr');
                tr.innerHTML = `<td>${node.name}</td><td>${node.last_poll ? node.last_poll : ''}</td><td>${node.status}</td>`;
                tr.className = node.status;
                tbody.appendChild(tr);
            });
        })
        .catch(() => {
            document.getElementById('status').textContent = 'Failed to fetch grid data.';
        });
}

fetchGrid();
setInterval(fetchGrid, 5000);
