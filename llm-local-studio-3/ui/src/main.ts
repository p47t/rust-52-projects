import './style.css';

const API_BASE = 'http://localhost:8080/v1';

interface ChatMessage {
  role: 'user' | 'assistant' | 'system';
  content: string;
}

interface ModelItem {
  id: string;
  object: string;
  created: number;
  owned_by: string;
}

interface ModelsResponse {
  object: string;
  data: ModelItem[];
}

// DOM Elements
const modelSelector = document.getElementById('model-selector') as HTMLSelectElement | null;
const currentModelName = document.getElementById('current-model-name') as HTMLElement | null;
const chatHistory = document.getElementById('chat-history') as HTMLElement | null;
const chatForm = document.getElementById('chat-form') as HTMLFormElement | null;
const chatInput = document.getElementById('chat-input') as HTMLTextAreaElement | null;
const sendButton = document.getElementById('send-button') as HTMLButtonElement | null;

// State
let selectedModel: string = '';
const messages: ChatMessage[] = [];
let isGenerating: boolean = false;

// Initialize
async function init() {
  await fetchModels();
  
  if (!chatInput || !sendButton || !modelSelector || !chatForm) return;

  // Auto-resize textarea
  chatInput.addEventListener('input', function(this: HTMLTextAreaElement) {
    this.style.height = 'auto';
    this.style.height = (this.scrollHeight) + 'px';
    if (this.value.trim() !== '') {
      sendButton.removeAttribute('disabled');
    } else {
      sendButton.setAttribute('disabled', 'true');
    }
  });

  chatInput.addEventListener('keydown', function(this: HTMLTextAreaElement, e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      if (!isGenerating && this.value.trim() !== '') {
        chatForm.dispatchEvent(new Event('submit'));
      }
    }
  });

  modelSelector.addEventListener('change', (e: Event) => {
    const target = e.target as HTMLSelectElement;
    selectedModel = target.value;
    if (currentModelName) {
      currentModelName.textContent = selectedModel;
    }
  });

  chatForm.addEventListener('submit', handleFormSubmit);
}

// Fetch available models
async function fetchModels() {
  if (!modelSelector) return;
  
  try {
    const response = await fetch(`${API_BASE}/models`);
    if (!response.ok) throw new Error('Failed to fetch models');
    
    const data: ModelsResponse = await response.json();
    modelSelector.innerHTML = '';
    
    if (data.data.length === 0) {
      const option = document.createElement('option');
      option.value = "";
      option.textContent = "No models available";
      option.disabled = true;
      option.selected = true;
      modelSelector.appendChild(option);
      return;
    }

    data.data.forEach((model, index) => {
      const option = document.createElement('option');
      option.value = model.id;
      option.textContent = model.id;
      if (index === 0) {
        option.selected = true;
        selectedModel = model.id;
        if (currentModelName) {
          currentModelName.textContent = model.id;
        }
      }
      modelSelector.appendChild(option);
    });
  } catch (error) {
    console.error('Error fetching models:', error);
    modelSelector.innerHTML = '<option value="" disabled selected>Error loading models</option>';
  }
}

// Handle form submission
async function handleFormSubmit(e: Event) {
  e.preventDefault();
  
  if (!chatInput || !sendButton || isGenerating) return;
  
  const content = chatInput.value.trim();
  if (!content) return;

  // Add user message
  messages.push({ role: 'user', content });
  appendMessage('user', content);
  
  // Reset input
  chatInput.value = '';
  chatInput.style.height = 'auto';
  sendButton.setAttribute('disabled', 'true');
  
  // Remove welcome message if exists
  const welcomeMsg = document.querySelector('.welcome-message');
  if (welcomeMsg) welcomeMsg.remove();

  await generateResponse();
}

// Append message to UI
function appendMessage(role: 'user' | 'assistant' | 'system', content: string): HTMLElement {
  const msgEl = document.createElement('div');
  msgEl.className = `message ${role}`;
  
  const contentEl = document.createElement('div');
  contentEl.className = 'message-content';
  contentEl.textContent = content; // Using textContent prevents XSS
  
  msgEl.appendChild(contentEl);
  
  if (chatHistory) {
    chatHistory.appendChild(msgEl);
    chatHistory.scrollTop = chatHistory.scrollHeight;
  }
  
  return contentEl;
}

// Generate response via SSE
async function generateResponse() {
  if (!chatInput || !modelSelector || !chatHistory) return;

  isGenerating = true;
  chatInput.setAttribute('disabled', 'true');
  modelSelector.setAttribute('disabled', 'true');

  // Create an empty AI message to stream into
  const msgEl = document.createElement('div');
  msgEl.className = 'message ai';
  const contentEl = document.createElement('div');
  contentEl.className = 'message-content';
  
  const cursor = document.createElement('span');
  cursor.className = 'cursor-blink';
  
  msgEl.appendChild(contentEl);
  msgEl.appendChild(cursor);
  chatHistory.appendChild(msgEl);
  
  let aiContent = '';

  try {
    const response = await fetch(`${API_BASE}/chat/completions`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        model: selectedModel || 'default',
        messages: messages,
        stream: true,
        max_tokens: 512
      })
    });

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    if (!response.body) {
      throw new Error('Response body is null');
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder('utf-8');

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      const chunk = decoder.decode(value, { stream: true });
      const lines = chunk.split('\n');

      for (const line of lines) {
        if (line.startsWith('data: ')) {
          const dataStr = line.replace('data: ', '').trim();
          if (dataStr === '[DONE]') continue;
          if (dataStr) {
            try {
              const data = JSON.parse(dataStr);
              if (data.choices && data.choices[0] && data.choices[0].delta && data.choices[0].delta.content) {
                aiContent += data.choices[0].delta.content;
                contentEl.textContent = aiContent;
                chatHistory.scrollTop = chatHistory.scrollHeight;
              }
            } catch (e) {
              console.error('Error parsing SSE data:', e, dataStr);
            }
          }
        }
      }
    }
    
    messages.push({ role: 'assistant', content: aiContent });

  } catch (error) {
    console.error('Error generating response:', error);
    contentEl.textContent += '\n\n[Error communicating with inference engine]';
    contentEl.style.color = '#ef4444';
  } finally {
    isGenerating = false;
    cursor.remove();
    chatInput.removeAttribute('disabled');
    modelSelector.removeAttribute('disabled');
    chatInput.focus();
    chatHistory.scrollTop = chatHistory.scrollHeight;
  }
}

// Start app
document.addEventListener('DOMContentLoaded', init);
