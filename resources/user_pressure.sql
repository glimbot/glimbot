SELECT SUM(pressure)
FROM messages
WHERE user = :uid
AND unix_time >= :since;