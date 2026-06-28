/**
 * Integration snippet for main.js to add compressed-step review
 * 
 * Add this after importing CompressionReview:
 * import { CompressionReview } from './compression-review.js';
 * 
 * And add buttons/handlers as shown below
 */

// Create a review modal for showing compressed steps
const reviewModal = document.createElement('div');
reviewModal.id = 'review-modal';
reviewModal.className = 'modal';
reviewModal.style.cssText = 'display: none; position: fixed; top: 0; left: 0; width: 100%; height: 100%; background: rgba(0,0,0,0.5); z-index: 1050; align-items: center; justify-content: center;';
const reviewContent = document.createElement('div');
reviewContent.className = 'modal-content review-panel';
reviewContent.setAttribute('role', 'dialog');
reviewContent.setAttribute('aria-modal', 'true');
reviewContent.setAttribute('aria-label', 'Workflow review');
reviewContent.setAttribute('tabindex', '-1');
reviewContent.style.cssText = 'background: var(--bg-soft); border: 1px solid var(--border); border-radius: 16px; padding: 24px; width: min(700px, 92vw); max-height: 80vh; overflow-y: auto; color: var(--text);';
reviewModal.appendChild(reviewContent);
document.body.appendChild(reviewModal);

const compressionReview = new CompressionReview('review-modal-compression');

// Add review button to workflow panel
document.addEventListener('DOMContentLoaded', () => {
  const workflowSection = document.querySelector('[aria-labelledby="workflow-heading"]');
  if (workflowSection) {
    const btnRow = workflowSection.querySelector('.btn-row');
    if (btnRow) {
      const reviewBtn = document.createElement('button');
      reviewBtn.className = 'btn btn--ghost btn--small';
      reviewBtn.id = 'reviewBtn';
      reviewBtn.textContent = 'Review Steps';
      reviewBtn.disabled = true;
      btnRow.appendChild(reviewBtn);

      reviewBtn.addEventListener('click', async () => {
        try {
          reviewBtn.disabled = true;
          reviewBtn.textContent = 'Reviewing...';

          // Compress the current recorded events
          const report = await compressionReview.compress(recordedEvents);

          // Show the review modal
          const header = document.createElement('div');
          header.className = 'review-header';
          header.innerHTML = `
            <h3>Workflow Review — ${report.compressed_step_count} steps</h3>
            <div class="review-actions">
              <button class="btn btn--ghost btn--small" id="closeReviewBtn">Close</button>
            </div>
          `;

          const container = document.createElement('div');
          container.id = 'review-modal-compression';

          reviewContent.innerHTML = '';
          reviewContent.appendChild(header);
          reviewContent.appendChild(container);

          compressionReview.render();

          // Add replay button below review
          const actions = document.createElement('div');
          actions.style.cssText = 'display: flex; gap: 8px; margin-top: 16px; border-top: 1px solid var(--border); padding-top: 16px;';
          actions.innerHTML = `
            <button class="btn btn--primary btn-review-replay" id="replayFromReviewBtn">Approve & Replay</button>
            <button class="btn btn--ghost btn-review-cancel" id="cancelReviewBtn">Cancel</button>
          `;
          reviewContent.appendChild(actions);

          reviewModal.style.display = 'flex';

          document.getElementById('closeReviewBtn').addEventListener('click', () => {
            reviewModal.style.display = 'none';
          });

          document.getElementById('replayFromReviewBtn').addEventListener('click', () => {
            reviewModal.style.display = 'none';
            // Trigger replay
            if (recordedEvents.length > 0) {
              replayRecordedEvents();
            }
          });

          document.getElementById('cancelReviewBtn').addEventListener('click', () => {
            reviewModal.style.display = 'none';
          });
        } catch (err) {
          showNotification(`Review failed: ${err.message}`, 'error');
          reviewBtn.disabled = false;
          reviewBtn.textContent = 'Review Steps';
        }
      });
    }
  }
});

// Enable review button when events are recorded
function updateReviewButton() {
  const reviewBtn = document.getElementById('reviewBtn');
  if (reviewBtn) {
    reviewBtn.disabled = recordedEvents.length === 0;
  }
}

// Hook into existing recording flow
const originalStopRecording = window.stopRecording;
window.stopRecording = function(...args) {
  const result = originalStopRecording?.apply(this, args);
  updateReviewButton();
  return result;
};
