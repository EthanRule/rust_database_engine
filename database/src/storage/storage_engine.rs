// 🎯 Real-World Analogy
// Think of it like a library with a smart lending system:

// Pages = Books on shelves
// Buffer Pool = Reading room (limited space, keeps popular books)
// Pinning = Checking out a book to your table
// Slots = Page numbers within each book
// Dirty = You wrote notes in the margins (needs to be saved)
// Unpinning = Returning the book (clean or with notes to be filed)
// TODO: Consider adding a tombstone Vacuum

use crate::{
    storage::{buffer_pool::BufferPool, file::DatabaseFile, page_layout::PageLayout}, 
    Document,
    document::bson::serialize_document
};
use std::path::Path;
use anyhow::Result;

#[derive(Debug)]
pub struct DocumentId {
    page_id: u32,
    slot_id: u16,
}

impl DocumentId {
    /// Create a new DocumentId
    pub fn new(page_id: u32, slot_id: u16) -> Self {
        Self { page_id, slot_id }
    }
    
    /// Get the page ID where the document is stored
    pub fn page_id(&self) -> u32 {
        self.page_id
    }
    
    /// Get the slot ID within the page where the document is stored
    pub fn slot_id(&self) -> u16 {
        self.slot_id
    }
}

pub struct StorageEngine {
    database_file: DatabaseFile,
    buffer_pool: BufferPool,
}

impl StorageEngine {
    pub fn new(database_path: &Path, buffer_pool_size: usize) -> Result<Self> {
        let database_file = DatabaseFile::open(database_path)?;
        let buffer_pool = BufferPool::new(buffer_pool_size);
        Ok(Self {
            database_file,
            buffer_pool,
        })
    }

    pub fn insert_document(&mut self, document: &Document) -> Result<DocumentId> {
        // 1. Serialize the document to BSON bytes
        let document_bytes = serialize_document(document)
            .map_err(|e| anyhow::anyhow!("Failed to serialize document: {}", e))?;
        let document_size = document_bytes.len();

        // 2. Try to find an existing page with enough free space
        let page_ids = self.buffer_pool.get_all_page_ids();
        for page_id in page_ids {
            // Pin the page to get mutable access
            if let Ok(page) = self.buffer_pool.pin_page(page_id) {
                let free_space = page.get_free_space() as usize;
                
                // Check if document can fit in this page
                if document_size <= free_space {
                    // Insert the document using PageLayout
                    match PageLayout::insert_document(page, &document_bytes) {
                        Ok(slot_id) => {
                            // Mark the page as dirty and unpin it
                            self.buffer_pool.unpin_page(page_id, true); // true = is_dirty
                            return Ok(DocumentId { 
                                page_id: page_id as u32, 
                                slot_id 
                            });
                        }
                        Err(_) => {
                            // Failed to insert, unpin the page without marking dirty
                            self.buffer_pool.unpin_page(page_id, false);
                            continue;
                        }
                    }
                }
                // Page doesn't have enough space, unpin it
                self.buffer_pool.unpin_page(page_id, false);
            }
        }

        // 3. No existing page has enough space, need to create a new page
        // For now, we'll return an error since page allocation isn't implemented yet
        Err(anyhow::anyhow!(
            "No existing page has sufficient space ({} bytes needed) and new page allocation is not yet implemented", 
            document_size
        ))
    }
}