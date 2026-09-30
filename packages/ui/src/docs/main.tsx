import { createRoot } from 'react-dom/client'
import { Gallery } from './Gallery'

const host = document.getElementById('gallery')
if (host === null) throw new Error('docs: #gallery is missing from index.html')

createRoot(host).render(<Gallery />)
