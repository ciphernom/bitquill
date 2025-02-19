import 'core-js/stable';
import 'regenerator-runtime/runtime';
console.log('Starting to load script...');

console.log('Starting import of WASM module...');
import('../pkg/bitquill_wasm')
  .then(async wasm => {
    console.log('WASM module loaded, full contents:', wasm);
    console.log('Available WASM exports:', Object.keys(wasm));
    window.wasmModule = wasm;

    if (typeof wasm.default === 'function') {
      console.log('Found default initializer, calling it...');
      try {
        await wasm.default();
        console.log('default() called successfully');
      } catch (err) {
        console.error('Error in default initialization:', err);
        throw err;
      }
    }

    if (typeof wasm.start === 'function') {
      console.log('Found start function, calling it...');
      try {
        wasm.start();
        console.log('start() called successfully');
      } catch (err) {
        console.error('Error in start():', err);
        throw err;
      }
    }

    console.log('About to call initializeApp...');
    return initializeApp(wasm);
  })
  .catch(err => {
    console.error('Detailed WASM load error:', {
      error: err,
      message: err.message,
      stack: err.stack,
      cause: err.cause,
      type: err.constructor.name
    });
  });
import '../static/styles/main.css';
import Quill from 'quill';
import 'quill/dist/quill.snow.css';

console.log('Imports completed');
const Delta = Quill.import('delta');
console.log('Script loaded');
console.log('About to define BitQuillApp');

class BitQuillApp {
  constructor(wasmModule) {
    this.wasmModule = wasmModule;
    
    // UI Elements
    this.lastKnownRange = null;
    this.editorContainer = document.getElementById('editor-container');
    this.newDocBtn = document.getElementById('new-doc-btn');
    this.verifyBtn = document.getElementById('verify-btn');
    this.serializeBtn = document.getElementById('serialize-btn');
    this.deserializeBtn = document.getElementById('deserialize-btn');
    this.timestampBtn = document.getElementById('timestamp-btn');
    this.fileInput = document.getElementById('file-input');
    this.statusDisplay = document.getElementById('status');
    this.spinner = document.getElementById('spinner');
    this.documentHashDisplay = document.getElementById('document-hash');
    this.contentDisplay = document.getElementById('content-display');
    this.serializedDataDisplay = document.getElementById('serialized-data');
    this.proofsList = document.getElementById('proofs-list');
    
    // Core components (will be set during init)
    this.quill = null;
    this.merkleTree = null;
    this.editAnalyzer = null;
    this.lastContent = null;
    
    // PoW configuration
    this.powConfig = {
      min: 1,
      max: 32,
      current: 1,
      adjustmentInterval: 201,
      targetEditInterval: 200,
      maxAdjustmentFactor: 4,
      windowSize: 50,
      minSampleSize: 5
    };
  }

  async init() {
    try {
      console.log('Init starting...');
      // Destructure the WASM exports for clarity
      const { MerkleTree, EditAnalyzer } = this.wasmModule;
      if (typeof MerkleTree !== 'function' || typeof EditAnalyzer !== 'function') {
        throw new Error('WASM exports not found or invalid.');
      }
      console.log('Creating MerkleTree and EditAnalyzer...');
      this.merkleTree = new MerkleTree();
      this.editAnalyzer = new EditAnalyzer();
      console.log('MerkleTree and EditAnalyzer created');

      console.log('Initializing Quill editor...');
      this.initializeQuillEditor();
      console.log('Quill editor initialized');

      console.log('Attaching event listeners...');
      this.attachEventListeners();
      console.log('Event listeners attached');

      console.log('Initializing document...');
      await this.initializeDocument();
      console.log('Document initialized');
      console.log('BitQuill initialized successfully');
    } catch (error) {
      console.error('Detailed init error:', {
        error,
        stack: error.stack,
        message: error.message,
        phase: 'init method'
      });
      this.updateStatus('Failed to initialize application', 'error');
      throw error;
    }
  }

  initializeQuillEditor() {
    this.quill = new Quill(this.editorContainer, {
      theme: 'snow',
      modules: {
        toolbar: {
          container: [
            [{ header: [1, 2, 3, false] }],
            ['bold', 'italic', 'underline'],
            ['link', 'image'],
            [{ list: 'ordered' }, { list: 'bullet' }],
            ['clean'],
            ['table']
          ],
          handlers: {
            'table': () => this.handleTableInsert()
          }
        }
      }
    });

    // Track selection changes
    this.quill.on('selection-change', (range, oldRange, source) => {
      if (range) {
        this.lastKnownRange = range;
      }
    });

    // Handle text changes
    this.quill.on('text-change', (delta, oldDelta, source) => {
      if (source === 'user') {
        this.handleEdit(delta, oldDelta);
      }
    });
  }

  async handleTableInsert() {
    const rows = parseInt(prompt('Number of rows:', '3')) || 3;
    const cols = parseInt(prompt('Number of columns:', '3')) || 3;
    
    const range = this.quill.getSelection(true);
    if (!range) return;

    let delta = new Delta().retain(range.index);
    delta = delta.insert('\n', { table: true });
    for (let i = 0; i < rows; i++) {
      for (let j = 0; j < cols; j++) {
        delta = delta.insert('Cell', { 'table-cell': true });
      }
      delta = delta.insert('\n', { 'table-row': true });
    }
    delta = delta.insert('\n');
    this.quill.updateContents(delta, 'user');
    this.quill.setSelection(range.index + 1, 0);
  }

  attachEventListeners() {
    this.newDocBtn.addEventListener('click', () => this.newDocument());
    this.verifyBtn.addEventListener('click', () => this.verifyDocument());
    this.serializeBtn.addEventListener('click', () => this.serializeDocument());
    this.deserializeBtn.addEventListener('click', () => this.fileInput.click());
    this.timestampBtn.addEventListener('click', () => this.manualTimestamp());
    this.fileInput.addEventListener('change', (event) => this.deserializeDocument(event));
  }

  async initializeDocument() {
    try {
      const initialDelta = this.quill.getContents();
      const timestamp = Date.now();
      const metadata = { timestamp, isGenesis: true };
      try {
        await this.merkleTree.add_leaf(JSON.stringify(initialDelta), JSON.stringify(metadata));
      } catch (e) {
        if (!e.toString().includes('invalid type: JsValue')) {
          throw e;
        }
      }
      this.lastContent = initialDelta;
      this.updateStatus('Document initialized', 'success');
      this.verifyBtn.disabled = false;
      await this.updateAllDisplays();
    } catch (error) {
      console.error('Document initialization error:', error);
      this.updateStatus(`Initialization failed: ${error.message}`, 'error');
    }
  }

    async handleEdit(delta, oldDelta) {
        const currentDelta = this.quill.getContents();
        const timestamp = Date.now();
        try {
            // Ensure proper Delta object with formatting
            const deltaObj = delta instanceof Delta ? delta : new Delta(delta);
            const deltaStr = JSON.stringify(deltaObj);
            
            console.log('Processing edit with delta:', deltaStr);

            // Run edit analysis
            let editAnalysis;
            try {
                const lastContentStr = JSON.stringify(this.lastContent);
                const rawEditAnalysis = await this.editAnalyzer.record_edit(
                    deltaStr,
                    lastContentStr,
                    timestamp
                );
                
                // Handle different response types
                if (typeof rawEditAnalysis === 'string') {
                    editAnalysis = JSON.parse(rawEditAnalysis);
                } else if (rawEditAnalysis instanceof Map) {
                    editAnalysis = Object.fromEntries(rawEditAnalysis);
                } else {
                    editAnalysis = rawEditAnalysis;
                }
            } catch (e) {
                console.error('Edit analysis failed:', e);
                editAnalysis = { 
                    isValid: true, 
                    patterns: ['Analysis error, proceeding with caution']
                };
            }

            // Validate analysis results
            if (!editAnalysis || typeof editAnalysis.isValid === 'undefined') {
                editAnalysis = { isValid: true, patterns: ['Default pattern'] };
            }
            if (!editAnalysis.isValid) {
                const patterns = Array.isArray(editAnalysis.patterns) 
                    ? editAnalysis.patterns 
                    : ['Unknown suspicious pattern'];
                throw new Error(`Suspicious edit pattern: ${patterns.join(', ')}`);
            }

            // Compute PoW with formatting preserved
            const powResult = await this.merkleTree.perform_pow(deltaStr, this.powConfig.current);

            // Build metadata and add leaf
            const metadata = {
                timestamp,
                powResult,
                editStats: editAnalysis.editStats,
                formatting: deltaObj.ops.some(op => op.attributes)
            };
            
            await this.merkleTree.add_leaf(
                deltaStr,
                JSON.stringify(metadata)
            );

            // Update state
            this.lastContent = currentDelta;
            
            // Adjust difficulty if needed
            if (editAnalysis && editAnalysis.totalEdits && 
                editAnalysis.totalEdits % this.powConfig.adjustmentInterval === 0) {
                this.adjustPowDifficulty();
            }

            this.updateUIAfterEdit('Edit recorded successfully', 'success');
            await this.updateAllDisplays();

        } catch (error) {
            console.error('Edit processing error:', error);
            this.updateStatus(error.message, 'error');
            
            // Restore previous state if available
            if (oldDelta) {
                this.quill.setContents(oldDelta, 'silent');
                this.lastContent = oldDelta;
            }
        }
    }

  async adjustPowDifficulty() {
    const stats = await this.editAnalyzer.get_edit_stats();
    if (!stats.geometricMeanInterval) return;
    let adjustmentFactor = this.powConfig.targetEditInterval / stats.geometricMeanInterval;
    adjustmentFactor = Math.min(
      Math.max(adjustmentFactor, 1 / this.powConfig.maxAdjustmentFactor),
      this.powConfig.maxAdjustmentFactor
    );
    const newDifficulty = Math.round(this.powConfig.current * adjustmentFactor);
    this.powConfig.current = Math.min(
      Math.max(this.powConfig.min, newDifficulty),
      this.powConfig.max
    );
    this.updateStatus(
      `Difficulty adjusted to ${this.powConfig.current} (mean interval: ${Math.round(stats.geometricMeanInterval)}ms)`,
      'info'
    );
  }

  async manualTimestamp() {
    try {
      this.updateStatus('Creating manual timestamp...', 'info');
      const result = await this.merkleTree.manual_timestamp();
      if (result) {
        this.updateStatus('Manual timestamp created successfully', 'success');
        await this.updateAllDisplays();
      } else {
        this.updateStatus('Failed to create timestamp', 'error');
      }
    } catch (error) {
      console.error('Timestamp error:', error);
      this.updateStatus('Failed to create timestamp', 'error');
    }
  }

  async newDocument() {
    try {
      this.quill.setText('');
      this.merkleTree = new this.wasmModule.MerkleTree();
      this.editAnalyzer = new this.wasmModule.EditAnalyzer();
      this.lastContent = this.quill.getContents();
      this.powConfig.current = this.powConfig.min;
      await this.initializeDocument();
      this.updateUIAfterEdit('New document created', 'success');
      await this.updateAllDisplays();
    } catch (error) {
      console.error('Error creating new document:', error);
      this.updateStatus('Failed to create new document', 'error');
    }
  }

  async serializeDocument() {
    try {
      this.disableUI();
      this.updateStatus('Serializing document...', 'info');
      this.spinner.style.display = 'inline-block';
      const serializedJson = await this.merkleTree.serialize();
      this.serializedDataDisplay.textContent = serializedJson;
      const jsonBytes = new TextEncoder().encode(serializedJson);
      try {
        const compressedStream = new Blob([jsonBytes]).stream().pipeThrough(new CompressionStream('gzip'));
        const compressedBlob = await new Response(compressedStream).blob();
        const compressedData = new Uint8Array(await compressedBlob.arrayBuffer());
        const blob = new Blob([compressedData], { type: 'application/x-bqlz' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `bitquill_document_${Date.now()}.bqlz`;
        a.click();
        URL.revokeObjectURL(url);
        const compressionRatio = (compressedData.length / jsonBytes.length * 100).toFixed(2);
        this.updateStatus(`Document serialized successfully (${compressionRatio}% of original size)`, 'success');
      } catch (error) {
        console.error('Compression error:', error);
        const blob = new Blob([serializedJson], { type: 'application/x-bql' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `bitquill_document_${Date.now()}.bql`;
        a.click();
        URL.revokeObjectURL(url);
        this.updateStatus('Document serialized successfully (uncompressed)', 'success');
      }
    } catch (error) {
      console.error('Serialization error:', error);
      this.updateStatus(`Serialization failed: ${error.message}`, 'error');
    } finally {
      this.spinner.style.display = 'none';
      this.enableUI();
    }
  }

  async deserializeDocument(event) {
    const file = event.target.files[0];
    if (!file) {
      this.updateStatus('No file selected', 'error');
      return;
    }
    try {
      this.disableUI();
      this.updateStatus('Deserializing document...', 'info');
      this.spinner.style.display = 'inline-block';
      let content;
      if (file.type === 'application/x-bqlz' || file.name.endsWith('.bqlz')) {
        const compressedStream = file.stream().pipeThrough(new DecompressionStream('gzip'));
        const decompressedBlob = await new Response(compressedStream).blob();
        content = await decompressedBlob.text();
      } else {
        content = await file.text();
      }
      const success = await this.merkleTree.deserialize(content);
      if (success) {
        let composedDelta = await this.merkleTree.get_current_content();
        if (typeof composedDelta === 'string') {
          composedDelta = JSON.parse(composedDelta);
        }
        // Directly set contents using the deserialized delta object.
        this.quill.setContents(composedDelta, 'silent');
        this.lastContent = composedDelta;
        this.updateStatus('Document deserialized successfully', 'success');
        await this.updateAllDisplays();
      } else {
        throw new Error('Failed to deserialize document');
      }
    } catch (error) {
      console.error('Deserialization error:', error);
      this.updateStatus(`Deserialization failed: ${error.message}`, 'error');
    } finally {
      this.spinner.style.display = 'none';
      this.enableUI();
      this.fileInput.value = '';
    }
  }

  async verifyDocument() {
    try {
      this.disableUI();
      this.updateStatus('Verifying document...', 'info');
      this.spinner.style.display = 'inline-block';
      const history = await this.merkleTree.get_history();
      let verifiedTimestamps = 0;
      for (let i = 0; i < history.length; i++) {
        const verification = await this.merkleTree.verify_proof(i);
        console.log(`Verification result for edit ${i}:`, verification);
        const verificationObj = verification instanceof Map ? Object.fromEntries(verification) : verification;
        if (!verificationObj.valid) {
          if (i === 0 && verificationObj.error === "Missing proof for non-genesis block") {
            continue;
          }
          console.error('Verification details:', {
            editIndex: i,
            verification: verificationObj,
            historyEntry: history[i]
          });
          this.updateStatus(`Verification failed at edit ${i + 1}: ${verificationObj.error}`, 'error');
          return;
        }
        if (verificationObj.timestamp) {
          verifiedTimestamps++;
        }
      }
      const message = verifiedTimestamps > 0 ?
        `Document verified with ${verifiedTimestamps} timestamp(s)` :
        'Document verified (no timestamps yet)';
      this.updateStatus(message, 'success');
    } catch (error) {
      console.error('Verification error:', error);
      this.updateStatus(`Verification failed: ${error.message}`, 'error');
    } finally {
      this.spinner.style.display = 'none';
      this.enableUI();
    }
  }

  async updateUIAfterEdit(message, type) {
    this.updateStatus(message, type);
    await this.updateAllDisplays();
  }

async updateAllDisplays() {
    try {
        // Preserve current selection
        const selection = this.quill.getSelection();
        
        // Get the composed content from Merkle tree
        let composedDelta = await this.merkleTree.get_current_content();
        
        // Handle string response
        if (typeof composedDelta === 'string') {
            try {
                composedDelta = JSON.parse(composedDelta);
            } catch (e) {
                console.error('Failed to parse composedDelta:', e);
                throw new Error('Invalid delta format received from Merkle tree');
            }
        }

        // Validate delta structure
        if (!composedDelta || !composedDelta.ops) {
            console.error('Invalid delta structure:', composedDelta);
            throw new Error('Invalid delta structure received from Merkle tree');
        }

        // Convert to proper Delta object preserving formatting
        const delta = new Delta(composedDelta);

        // Validate delta operations
        for (const op of delta.ops) {
            if (!op.insert && !op.delete && !op.retain) {
                console.error('Invalid operation:', op);
                throw new Error('Invalid operation in composed delta');
            }
        }

        // Update editor contents
        console.log('Updating editor with delta:', delta);
        this.quill.setContents(delta, 'api');

        // Restore selection if it existed
        if (selection) {
            this.quill.setSelection(selection);
        }

        // Update other UI elements
        const history = await this.merkleTree.get_history();
        if (history.length > 0) {
            const latestIndex = history.length - 1;
            const proofJson = await this.merkleTree.get_proof(latestIndex);
            const proofHash = await this.merkleTree.compute_hash_js(proofJson);
            this.documentHashDisplay.textContent = proofHash;
            await this.updateProofsList(history);
        } else {
            this.documentHashDisplay.textContent = 'No proof yet...';
            this.proofsList.innerHTML = '';
        }

    } catch (error) {
        console.error('Display update error:', error);
        this.updateStatus('Failed to update display: ' + error.message, 'error');
    }
}

  async updateProofsList(history) {
    const proofs = await Promise.all(history.map((_, index) => this.merkleTree.get_proof(index)));
    this.proofsList.innerHTML = proofs.map((proofJson, index) => {
      return `
          <li class="p-2 bg-gray-100 rounded">
            <div><strong>Edit ${index + 1}</strong></div>
            <div>Proof:</div>
            <pre>${proofJson}</pre>
          </li>
        `;
    }).join('');
  }

  updateStatus(message, type) {
    this.statusDisplay.textContent = message;
    this.statusDisplay.className = `status ${type}`;
  }

  disableUI() {
    this.verifyBtn.disabled = true;
    this.serializeBtn.disabled = true;
    this.deserializeBtn.disabled = true;
    this.timestampBtn.disabled = true;
    this.newDocBtn.disabled = true;
  }

  enableUI() {
    this.verifyBtn.disabled = false;
    this.serializeBtn.disabled = false;
    this.deserializeBtn.disabled = false;
    this.timestampBtn.disabled = false;
    this.newDocBtn.disabled = false;
  }
}

async function initializeApp(wasmModule) {
  const { MerkleTree, EditAnalyzer } = wasmModule;
  console.log('MerkleTree:', MerkleTree);
  console.log('EditAnalyzer:', EditAnalyzer);
  if (typeof MerkleTree !== 'function' || typeof EditAnalyzer !== 'function') {
    throw new Error('WASM exports are not available as functions.');
  }
  const app = new BitQuillApp(wasmModule);
  app.merkleTree = new MerkleTree();
  app.editAnalyzer = new EditAnalyzer();
  await app.init();

  window.addEventListener('unhandledrejection', event => {
    if (event.reason.includes('WASM')) {
      console.error('WASM operation failed:', event.reason);
      app.updateStatus('Operation failed - WASM error', 'error');
      event.preventDefault();
    }
  });

  window.addEventListener('unload', () => {
    if (app.merkleTree) app.merkleTree.free();
    if (app.editAnalyzer) app.editAnalyzer.free();
  });
}
