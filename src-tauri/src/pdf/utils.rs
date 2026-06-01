use lopdf::{Document, Error as LopdfError, Object, ObjectId};
use std::collections::{HashMap, HashSet, VecDeque};

pub fn manual_deep_copy(
    source_doc: &Document,
    target_doc: &mut Document,
    ids_to_copy: &[ObjectId],
) -> Result<HashMap<ObjectId, ObjectId>, LopdfError> {
    let mut id_map: HashMap<ObjectId, ObjectId> = HashMap::new();
    let mut queue: VecDeque<ObjectId> = ids_to_copy.iter().cloned().collect();
    let mut processed: HashSet<ObjectId> = ids_to_copy.iter().cloned().collect();
    let mut loop_count = 0;
    let max_loops = (source_doc.objects.len() + ids_to_copy.len()) * 2; // Safety limit

    while let Some(old_id) = queue.pop_front() {
        loop_count += 1;
        if loop_count > max_loops {
            // Prevent potential infinite loops in malformed PDFs
            return Err(LopdfError::Syntax(format!(
                "Deep copy loop exceeded limit ({})",
                max_loops
            )));
        }

        if id_map.contains_key(&old_id) {
            continue; // Already copied and mapped
        }

        // Get original object, skip if missing but allow process to continue for others
        let source_object = match source_doc.get_object(old_id) {
            Ok(obj) => obj,
            Err(e) => {
                eprintln!("Warning: Failed to get source object {:?} during deep copy: {}. Skipping object.", old_id, e);
                continue; // Skip this missing object
            }
        };

        // Find references in the original object *before* cloning
        find_references_recursive(source_object, &mut queue, &mut processed)?;

        // Clone the object and add it to the target document
        let cloned_object = source_object.clone();
        let new_id = target_doc.add_object(cloned_object);
        id_map.insert(old_id, new_id);
    }

    // --- Second Pass: Update references in copied objects ---
    for (_old_id, new_id) in &id_map {
        // Iterate only over successfully copied objects
        match target_doc.get_object_mut(*new_id) {
            Ok(target_object) => {
                // If updating references fails, propagate the error
                update_references_recursive(target_object, &id_map)?;
            }
            Err(e) => {
                // This indicates an object that *was* mapped couldn't be retrieved.
                // This is more critical than failing to get an object in Pass 1.
                eprintln!(
                    "ERROR: Could not get mapped object {:?} for ref update (Pass 2): {}",
                    new_id, e
                );
                // Propagate as an error because subsequent state is unreliable.
                return Err(LopdfError::Syntax(format!(
                    "Failed to retrieve copied object {:?} during reference update",
                    new_id
                )));
            }
        }
    }

    Ok(id_map)
}

pub fn find_references_recursive(
    object: &Object,
    queue: &mut VecDeque<ObjectId>,
    processed: &mut HashSet<ObjectId>,
) -> Result<(), LopdfError> {
    match object {
        Object::Reference(id) => {
            if processed.insert(*id) {
                queue.push_back(*id);
            }
        }
        Object::Array(arr) => {
            for item in arr {
                find_references_recursive(item, queue, processed)?;
            }
        }
        Object::Dictionary(dict) => {
            for (_, value) in dict.iter() {
                find_references_recursive(value, queue, processed)?;
            }
        }
        Object::Stream(stream) => {
            for (_, value) in stream.dict.iter() {
                find_references_recursive(value, queue, processed)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn update_references_recursive(
    object: &mut Object,
    id_map: &HashMap<ObjectId, ObjectId>,
) -> Result<(), LopdfError> {
    match object {
        Object::Reference(ref mut old_id_ref) => {
            // If the referenced ObjectId exists in the map, update it to the new ObjectId
            if let Some(new_id) = id_map.get(old_id_ref) {
                *old_id_ref = *new_id;
            }
            // If not in map, it references something outside the copied scope; leave it.
        }
        Object::Array(arr) => {
            // Recursively update references in each element of the array
            for item in arr.iter_mut() {
                // Use iter_mut() to allow modification
                update_references_recursive(item, id_map)?;
            }
        }
        Object::Dictionary(dict) => {
            // Recursively update references in each value of the dictionary
            for (_key, value) in dict.iter_mut() {
                // Use iter_mut()
                update_references_recursive(value, id_map)?;
            }
        }
        Object::Stream(stream) => {
            // Recursively update references in the stream's dictionary values
            for (_key, value) in stream.dict.iter_mut() {
                // Use iter_mut()
                update_references_recursive(value, id_map)?;
            }
        }
        // Other types (Name, String, Integer, etc.) don't contain refs to update
        _ => {}
    }
    Ok(())
}
