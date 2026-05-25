import './style.css';

const API_BASE = 'http://localhost:8080/v1';

interface ChatMessage {
  role: 'user' | 'assistant' | 'system';
  content: string | any[];
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

// Audio Elements
const micButton = document.getElementById('mic-button') as HTMLButtonElement | null;
const audioMode = document.getElementById('audio-mode') as HTMLSelectElement | null;
const whisperSettings = document.getElementById('whisper-settings') as HTMLElement | null;
const audioTask = document.getElementById('audio-task') as HTMLSelectElement | null;
const audioLanguage = document.getElementById('audio-language') as HTMLSelectElement | null;

// State
let selectedModel: string = '';
const messages: ChatMessage[] = [];
let isGenerating: boolean = false;

// Recording State
let mediaRecorder: MediaRecorder | null = null;
let audioChunks: Blob[] = [];
let isRecording: boolean = false;

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

  if (micButton) {
    micButton.addEventListener('click', toggleRecording);
  }

  if (audioMode && whisperSettings) {
    audioMode.addEventListener('change', () => {
      if (audioMode.value === 'multimodal') {
        whisperSettings.style.display = 'none';
      } else {
        whisperSettings.style.display = 'block';
      }
    });
  }
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

// Toggle recording state
async function toggleRecording() {
  if (isRecording) {
    stopRecording();
  } else {
    await startRecording();
  }
}

// Start audio recording
async function startRecording() {
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    
    mediaRecorder = new MediaRecorder(stream, { mimeType: 'audio/webm' });
    audioChunks = [];

    mediaRecorder.ondataavailable = (event) => {
      if (event.data.size > 0) {
        audioChunks.push(event.data);
      }
    };

    mediaRecorder.onstop = async () => {
      const audioBlob = new Blob(audioChunks, { type: 'audio/webm' });
      stream.getTracks().forEach(track => track.stop());
      
      const mode = audioMode?.value || 'asr';
      if (mode === 'multimodal') {
        await handleAudioMultimodal(audioBlob);
      } else {
        await handleAudioUpload(audioBlob);
      }
    };

    mediaRecorder.start();
    isRecording = true;
    if (micButton) {
      micButton.classList.add('recording');
      micButton.title = "Stop Recording";
    }
  } catch (error) {
    console.error('Failed to access microphone:', error);
    alert('Could not access microphone. Please ensure microphone permissions are granted.');
  }
}

// Stop audio recording
function stopRecording() {
  if (mediaRecorder && isRecording) {
    mediaRecorder.stop();
    isRecording = false;
    if (micButton) {
      micButton.classList.remove('recording');
      micButton.title = "Record Voice";
    }
  }
}

// Upload recorded audio file
async function handleAudioUpload(audioBlob: Blob) {
  if (!chatInput || !sendButton) return;

  const task = audioTask?.value || 'transcribe';
  const language = audioLanguage?.value || 'auto';
  
  const endpoint = task === 'translate' ? 'translations' : 'transcriptions';
  const url = `${API_BASE}/audio/${endpoint}`;

  const formData = new FormData();
  formData.append('file', audioBlob, 'recording.webm');
  formData.append('model', 'whisper-1');
  if (language !== 'auto' && task !== 'translate') {
    formData.append('language', language);
  }

  // Update UI placeholder to show transcription state
  const originalPlaceholder = chatInput.placeholder;
  chatInput.placeholder = "Transcribing voice...";
  chatInput.disabled = true;
  if (micButton) micButton.disabled = true;

  try {
    const response = await fetch(url, {
      method: 'POST',
      body: formData,
    });

    if (!response.ok) {
      const errText = await response.text();
      throw new Error(`Failed to transcribe: ${errText}`);
    }

    const data = await response.json();
    if (data.text) {
      chatInput.value = data.text;
      chatInput.style.height = 'auto';
      chatInput.style.height = (chatInput.scrollHeight) + 'px';
      sendButton.removeAttribute('disabled');
    }
  } catch (error) {
    console.error('Transcription error:', error);
    alert(`Speech recognition failed: ${error instanceof Error ? error.message : String(error)}`);
  } finally {
    chatInput.placeholder = originalPlaceholder;
    chatInput.disabled = false;
    if (micButton) micButton.disabled = false;
    chatInput.focus();
  }
}

// Handle direct multimodal audio sending
async function handleAudioMultimodal(audioBlob: Blob) {
  if (!chatInput || !sendButton) return;

  const originalPlaceholder = chatInput.placeholder;
  chatInput.placeholder = "Encoding voice...";
  chatInput.disabled = true;
  if (micButton) micButton.disabled = true;

  try {
    const base64Data = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onloadend = () => {
        const result = reader.result as string;
        const base64 = result.split(',')[1];
        resolve(base64);
      };
      reader.onerror = reject;
      reader.readAsDataURL(audioBlob);
    });

    const textPrompt = chatInput.value.trim();
    const contentParts: any[] = [];
    if (textPrompt) {
      contentParts.push({ type: 'text', text: textPrompt });
    } else {
      contentParts.push({ type: 'text', text: 'Listen and respond to this audio input.' });
    }
    contentParts.push({
      type: 'input_audio',
      input_audio: {
        data: base64Data,
        format: 'wav'
      }
    });

    // Display user message in UI
    const displayPrompt = textPrompt ? `🎤 [Voice] ${textPrompt}` : `🎤 [Voice]`;
    appendMessage('user', displayPrompt);

    // Push the message into the state message list
    messages.push({
      role: 'user',
      content: contentParts
    });

    // Reset input fields
    chatInput.value = '';
    chatInput.style.height = 'auto';
    sendButton.setAttribute('disabled', 'true');
    
    const welcomeMsg = document.querySelector('.welcome-message');
    if (welcomeMsg) welcomeMsg.remove();

    // Trigger completion
    await generateResponse();

  } catch (error) {
    console.error('Multimodal audio error:', error);
    alert(`Failed to process voice input: ${error instanceof Error ? error.message : String(error)}`);
  } finally {
    chatInput.placeholder = originalPlaceholder;
    chatInput.disabled = false;
    if (micButton) micButton.disabled = false;
    chatInput.focus();
  }
}

// Start app
document.addEventListener('DOMContentLoaded', init);
