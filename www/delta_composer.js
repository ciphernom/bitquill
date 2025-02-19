// www/delta_composer.js
import Quill from 'quill';
const Delta = Quill.import('delta');

/**
 * Compose an array of delta objects using Quill’s built-in Delta.compose().
 *
 * @param {Array<Object>} deltas - An array of delta objects.
 * @returns {Object} - The composed delta.
 */
export function composeDeltas(deltas) {
    if (!Array.isArray(deltas)) {
        throw new Error('Expected array of deltas');
    }

    let composed = new Delta();

    for (const deltaObj of deltas) {
        try {
            // Handle null/undefined deltas
            if (!deltaObj) continue;

            // Ensure proper Delta object
            const delta = deltaObj instanceof Delta ? deltaObj : new Delta(deltaObj);

            // Handle empty delta edge case
            if (!delta.ops || delta.ops.length === 0) continue;

            // Preserve attributes during composition
            composed = composed.compose(delta);

            // Validate composed result has proper structure
            if (!composed.ops || !Array.isArray(composed.ops)) {
                throw new Error('Invalid composed delta structure');
            }

            // Validate each operation in composed result
            for (const op of composed.ops) {
                if (typeof op !== 'object') {
                    throw new Error('Invalid operation in composed delta');
                }
                if (!op.insert && !op.delete && !op.retain) {
                    throw new Error('Operation missing required properties');
                }
            }
        } catch (err) {
            console.error('Delta composition error:', {
                error: err,
                deltaObj,
                currentComposed: composed
            });
            throw err;
        }
    }

    // Ensure the final composed delta is valid
    try {
        new Delta(composed);
    } catch (err) {
        console.error('Final composition validation failed:', err);
        throw err;
    }

    return composed;
}
