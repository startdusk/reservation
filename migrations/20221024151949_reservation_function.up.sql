-- if user_id is null, find all reservations within during for the resource
-- if resource_id is null, find all reservations within during for the user
-- if both are null, find all reservations within during
-- if both set, find all reservations within during for the resource and user

CREATE OR REPLACE FUNCTION rsvp.query(
    uid text, 
    rid text, 
    during TSTZRANGE,
    status rsvp.reservation_status,
    page integer DEFAULT 1,
    is_desc BOOL DEFAULT FALSE,
    page_size INTEGER DEFAULT 10
) RETURNS TABLE (LIKE rsvp.reservations) 
AS $$ 
DECLARE
    _sql TEXT;
BEGIN
    -- format the query based on parameters
    _sql := format(
        'SELECT * FROM rsvp.reservations WHERE %L @> timespan AND status = %L AND %s ORDER BY LOWER(timespan) %s LIMIT %L::integer OFFSET %L::integer', 
        during,
        status,
        CASE
            WHEN uid IS NULL AND rid IS NULL THEN 'TRUE'
            WHEN uid IS NULL THEN 'resource_id = ' || QUOTE_LITERAL(rid)
            WHEN rid IS NULL THEN 'user_id = ' || QUOTE_LITERAL(uid)
            ELSE 'user_id = ' || QUOTE_LITERAL(uid) || ' AND resource_id = ' || QUOTE_LITERAL(rid)
        END,
        CASE
            WHEN is_desc THEN 'DESC'
            ELSE 'ASC'
        END,
        page_size,
        (page - 1) * page_size
    );

    -- log the sql
    RAISE NOTICE '%', _sql;

    -- execute the query
    RETURN QUERY EXECUTE _sql;
    -- Normally, you have to create new migrations to modify the database, 
    -- especially if you have deployed an online environment, 
    -- as you are now in the development learning stage, do so for comparison

    -- -- if both are null, find all reservations within during
    -- IF uid IS NULL AND rid IS NULL THEN
    --     RETURN QUERY SELECT * FROM rsvp.reservations WHERE timespan @> during;
    -- ELSEIF uid IS NULL THEN
    --     -- if user_id is null, find all reservations within during for the resource
    --     RETURN QUERY SELECT * FROM rsvp.reservations WHERE resource_id = rid AND during @> timespan;
    -- ELSEIF rid IS NULL THEN
    --     -- if resource_id is null, find all reservations within during for the user
    --     RETURN QUERY SELECT * FROM rsvp.reservations WHERE user_id = uid AND during @> timespan;
    -- ELSE
    --     -- if both set, find all reservations within during for the resource and user
    --     RETURN QUERY SELECT * FROM rsvp.reservations WHERE resource_id = rid AND user_id = uid AND during @> timespan;
    -- END IF;
END;
$$ LANGUAGE plpgsql;
