import React, { useEffect, useRef, useState } from 'react';
import L from 'leaflet';
import 'leaflet/dist/leaflet.css';
import './index.css'; // Include the CSS for the spinner

const MapComponent = () => {
    const mapRef = useRef(null); 
    const mapInstance = useRef(null); 
    const [imageUrl, setImageUrl] = useState(''); 
    const [selectedOption, setSelectedOption] = useState('hillshade'); 
    const [loading, setLoading] = useState(false); // Loading state

    useEffect(() => {
        mapInstance.current = L.map(mapRef.current).setView([45.102, 1.460], 13);//includes preferred loading coords and zoom
        L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
            maxZoom: 19,
            attribution: '© OpenStreetMap contributors'
        }).addTo(mapInstance.current);
        return () => {
            mapInstance.current.remove();
        };
    }, []);

    const updateBoundingBox = () => {
        const bounds = mapInstance.current.getBounds();
        const zoom = mapInstance.current.getZoom();
        return {
            ulx: bounds.getNorthWest().lng, // Upper Left X (longitude)
            uly: bounds.getNorthWest().lat, // Upper Left Y (latitude)
            lrx: bounds.getSouthEast().lng, // Lower Right X (longitude)
            lry: bounds.getSouthEast().lat,  // Lower Right Y (latitude)
            zoom_level: zoom,
            option: selectedOption
        };
    };

    const sendBboxDatandDisplayImage = async (bbox) => {
        setLoading(true); // Start loading spinner
        try {
            const response = await fetch('http://127.0.0.1:8000/api/image', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(bbox)
            });

            if (response.ok) {
                const data = await response.json();
                setImageUrl(data.image_path); // Set image URL
            } else {
                console.error('Failed to send bounding box. Status:', response.status);
            }
        } catch (error) {
            console.error('Error sending bounding box:', error);
        } finally {
            setLoading(false); // Stop loading spinner
        }
    };

    const handleGenerateClick = () => {
        const bbox = updateBoundingBox(); 
        sendBboxDatandDisplayImage(bbox); 
    };

    return (
        <div>
            <h1>Terrain Image Generator</h1>
            <div id="map" ref={mapRef}></div>

            <div className="button-radio-container">
                <div className="radio-options">
                    <label>
                        <input
                            type="radio"
                            value="hillshade"
                            checked={selectedOption === 'hillshade'}
                            onChange={() => setSelectedOption('hillshade')}
                        />
                        Hillshade
                    </label>
                    <label>
                        <input
                            type="radio"
                            value="contour"
                            checked={selectedOption === 'contour'}
                            onChange={() => setSelectedOption('contour')}
                        />
                        Contour
                    </label>
                </div>
                <button onClick={handleGenerateClick}>Generate</button>
            </div>

            {loading && (
                <div className="loading-spinner"></div>
            )}

            <div id="image-container">
                {imageUrl && <img id="image" src={imageUrl} alt="Generated" style={{ marginTop: '20px' }} />}
            </div>
        </div>
    );
};

export default MapComponent;
